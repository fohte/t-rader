use std::future::pending;
use std::{collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use chrono::Utc;
use core_application::{
    kata_exec::{ExecFile, ExecRequest, ExecResult, FakeKataExecutor, SharedKataExecutor},
    strategy_execution::{
        DatasetCaptureOptions, DatasetCaptureReport, DatasetPage, DatasetPageCompletion,
        DatasetProvider, DatasetProviderError, DatasetSnapshot, PayloadDefinition, PayloadKind,
        PayloadPage, PayloadRef, ReadPayloadQuery, SharedDatasetProvider,
    },
    strategy_scope::StrategyScope,
};
use gateway_postgres::entities::{
    custom_indicator, custom_indicator_evaluation, strategy_task,
    task_execution_definition_snapshot, task_execution_payload,
};
use indoc::indoc;
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::super::mcp_tool::call_tool_output_with_headers_and_id;
use super::{StrategyServer, tests_common};

const DATASET_REVISION: &str = "sample-revision-a";
const DATASET_DIGEST: &str = "b52cddda5a045faaee6394056f35404ef21a3b843edfbe739fc30fd52933c8ca";
const OPERATION_INPUT_DIGEST: &str =
    "e9551a737fab4cb4e4ad3eefdf6a95ab79ad7c934046f46f145bdaad56720d9b";
const OPERATION_OUTPUT_DIGEST: &str =
    "02e3edf64da0a6e80a2df605decdb64605cd8a66b71be923c8148c945955b615";
const OPERATION_FAILURE_DIGEST: &str =
    "6ee28d7049e92a25b270e9f8d805a635d27e957420d6bb1c4f6270a10515d804";
const TASK_START_SNAPSHOT_DIGEST: &str =
    "8b71de3f8e45dcb78cf9e14199658490a9bbe1b59a902c22b849b6386b921cbc";

#[derive(Clone, Copy)]
enum ProviderMode {
    Stable,
    ChangeRevision,
    ExceedPageLimit,
    Truncated,
    UnknownExpectedCount,
    MismatchedCompletionCount,
    MoreWithoutCursor,
}

struct SyntheticDatasetProvider {
    items: Vec<Value>,
    mode: ProviderMode,
}

struct WaitingDatasetProvider {
    started: Arc<tokio::sync::Notify>,
    wait_at: DatasetWaitStage,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DatasetWaitStage {
    Open,
    ReadPage,
}

#[async_trait]
impl DatasetProvider for WaitingDatasetProvider {
    async fn open(
        &self,
        _scope: StrategyScope,
        _request: Value,
    ) -> Result<DatasetSnapshot, DatasetProviderError> {
        if self.wait_at == DatasetWaitStage::Open {
            self.started.notify_one();
            pending().await
        }
        Ok(DatasetSnapshot {
            source_revision: DATASET_REVISION.into(),
            source_locator: None,
            source_metadata: Value::Null,
            schema: json!({"type": "array"}),
            captured_at: Utc::now().fixed_offset(),
            expected_count: None,
        })
    }

    async fn read_page(
        &self,
        _snapshot: &DatasetSnapshot,
        _cursor: Option<&str>,
        _limit: usize,
    ) -> Result<DatasetPage, DatasetProviderError> {
        if self.wait_at == DatasetWaitStage::ReadPage {
            self.started.notify_one();
            pending().await
        }
        Err(DatasetProviderError::Failed(
            "unexpected provider read".into(),
        ))
    }
}

impl SyntheticDatasetProvider {
    fn expected_count(&self) -> Option<u64> {
        match self.mode {
            ProviderMode::UnknownExpectedCount => None,
            _ => Some(self.items.len() as u64),
        }
    }

    fn completion_report(&self, received_count: usize) -> DatasetCaptureReport {
        let received_count = received_count as u64;
        DatasetCaptureReport {
            expected_count: self.expected_count(),
            received_count: received_count
                + u64::from(matches!(self.mode, ProviderMode::MismatchedCompletionCount)),
            missing_count: self.expected_count().map(|_| 0),
            unretrieved_count: Some(if matches!(self.mode, ProviderMode::Truncated) {
                self.items.len().saturating_sub(received_count as usize) as u64
            } else {
                0
            }),
        }
    }
}

#[async_trait]
impl DatasetProvider for SyntheticDatasetProvider {
    async fn open(
        &self,
        _scope: StrategyScope,
        _request: Value,
    ) -> Result<DatasetSnapshot, DatasetProviderError> {
        Ok(DatasetSnapshot {
            source_revision: DATASET_REVISION.into(),
            source_locator: Some(json!({"page_key": "sample-page"})),
            source_metadata: json!({"provider_tag": "synthetic"}),
            schema: json!({
                "type": "array",
                "items": {"type": "object", "properties": {"value": {"type": "integer"}},
                    "required": ["value"], "additionalProperties": false},
            }),
            captured_at: Utc::now().fixed_offset(),
            expected_count: self.expected_count(),
        })
    }

    async fn read_page(
        &self,
        snapshot: &DatasetSnapshot,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<DatasetPage, DatasetProviderError> {
        let start = cursor
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|error| DatasetProviderError::Failed(error.to_string()))?;
        let page_limit = match self.mode {
            ProviderMode::ExceedPageLimit if start == 0 => limit.saturating_add(1),
            _ => limit,
        };
        let end = start.saturating_add(page_limit).min(self.items.len());
        let source_revision = match self.mode {
            ProviderMode::ChangeRevision if start > 0 => "sample-revision-b",
            _ => snapshot.source_revision.as_str(),
        };
        let completion = match self.mode {
            ProviderMode::Truncated if start == 0 => {
                DatasetPageCompletion::Incomplete(self.completion_report(end))
            }
            ProviderMode::MoreWithoutCursor if start == 0 => DatasetPageCompletion::More,
            _ if end < self.items.len() => DatasetPageCompletion::More,
            _ => DatasetPageCompletion::Complete(self.completion_report(end)),
        };
        Ok(DatasetPage {
            source_revision: source_revision.into(),
            items: self.items[start..end].to_vec(),
            next_cursor: (end < self.items.len()
                && !matches!(
                    self.mode,
                    ProviderMode::Truncated | ProviderMode::MoreWithoutCursor
                ))
            .then(|| end.to_string()),
            completion,
        })
    }
}

async fn create_running_task(
    db: &gateway_postgres::DatabaseHandle,
    strategy_id: Uuid,
) -> (Uuid, String) {
    let task = tests_common::create_test_execution_task(db, strategy_id, "phases: []").await;
    (task.task_id, task.a2a_task_id)
}

#[backend_test_macros::database_test]
async fn mcp_task_snapshot_returns_fixed_definition_and_rejects_digest_mismatch(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (valid_task_id, valid_a2a_task_id) = create_running_task(&db, strategy_id).await;
    let (invalid_task_id, invalid_a2a_task_id) = create_running_task(&db, strategy_id).await;
    let (_, missing_a2a_task_id) = create_running_task(&db, strategy_id).await;
    let valid_execution_id = format!("{valid_a2a_task_id}:{}", Uuid::from_u128(709));
    let invalid_execution_id = format!("{invalid_a2a_task_id}:{}", Uuid::from_u128(710));
    let missing_execution_id = format!("{missing_a2a_task_id}:{}", Uuid::from_u128(711));
    let definition = json!({"phase": "synthetic"});
    for (task_id, digest) in [
        (valid_task_id, TASK_START_SNAPSHOT_DIGEST),
        (invalid_task_id, "demo-invalid-digest"),
    ] {
        task_execution_definition_snapshot::ActiveModel {
            task_id: Set(task_id),
            definition_key: Set("task_start".into()),
            definition: Set(definition.clone()),
            digest: Set(digest.into()),
            created_at: Set(Utc::now().fixed_offset()),
        }
        .insert(&db)
        .await
        .expect("insert synthetic definition snapshot");
    }

    let server = tests_common::build_server(db);
    let valid: Result<Value, String> = server
        .invoke::<_, Value>(
            "read_task_execution_snapshot",
            strategy_id,
            json!({}),
            Some(valid_execution_id),
            None,
        )
        .await
        .map(|response| response.into_value())
        .map_err(|error| error.message.to_string());
    let missing: Result<Value, String> = server
        .invoke::<_, Value>(
            "read_task_execution_snapshot",
            strategy_id,
            json!({}),
            Some(missing_execution_id),
            None,
        )
        .await
        .map(|response| response.into_value())
        .map_err(|error| error.message.to_string());
    let invalid: Result<Value, String> = server
        .invoke::<_, Value>(
            "read_task_execution_snapshot",
            strategy_id,
            json!({}),
            Some(invalid_execution_id),
            None,
        )
        .await
        .map(|response| response.into_value())
        .map_err(|error| error.message.to_string());

    let actual = (valid, missing, invalid);

    assert_eq!(
        actual,
        (
            Ok(json!({
                "definition": definition,
                "digest": TASK_START_SNAPSHOT_DIGEST,
            })),
            Ok(json!({"definition": null, "digest": null})),
            Err("strategy graph execution failed".into()),
        ),
    );
}

#[backend_test_macros::database_test]
async fn task_only_mcp_reads_authorize_snapshot_and_sealed_payload_with_fixed_deadline(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let other_strategy_id = tests_common::insert_strategy(&db, "other-sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let snapshot = task_execution_definition_snapshot::Entity::find()
        .filter(task_execution_definition_snapshot::Column::TaskId.eq(task_id))
        .filter(task_execution_definition_snapshot::Column::DefinitionKey.eq("task_start"))
        .one(&db)
        .await
        .expect("query task start snapshot")
        .expect("task start snapshot exists");
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone());
    let access = use_cases
        .strategy_executions()
        .authorize_execution(
            StrategyScope::from(strategy_id),
            &a2a_task_id,
            Some(Uuid::from_u128(703)),
        )
        .await
        .expect("authorize synthetic task");
    let payload = use_cases
        .strategy_executions()
        .write_payload(
            &access,
            PayloadDefinition {
                kind: PayloadKind::Output,
                source: "synthetic-operation".into(),
                source_revision: Some("sample-revision-a".into()),
                source_locator: None,
                source_metadata: json!({}),
                schema: json!({"type": "object"}),
            },
            json!({"value": {"items": [{"key": "synthetic"}]}}),
        )
        .await
        .expect("write sealed synthetic payload");
    let server = tests_common::build_server(db.clone());
    let task_only_id = Some(a2a_task_id.clone());
    let mut task_only_headers = HeaderMap::new();
    task_only_headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy ID"),
    );
    task_only_headers.insert(
        "x-execution-id",
        HeaderValue::from_str(&a2a_task_id).expect("valid task-only execution ID"),
    );
    task_only_headers.insert(
        "x-strategy-task-id",
        HeaderValue::from_str(&task_id.to_string()).expect("valid task UUID"),
    );
    let snapshot_read: Value = call_tool_output_with_headers_and_id(
        server.entrypoint_server(),
        "read_task_execution_snapshot",
        json!({}),
        rmcp::model::NumberOrString::String("same-task-read-request-id".into()),
        task_only_headers.clone(),
    )
    .await
    .expect("read task-only execution snapshot");
    let payload_page: Value = call_tool_output_with_headers_and_id(
        server.entrypoint_server(),
        "read_execution_payload",
        json!({
            "payload_id": payload.id,
            "pointer": "/value/items",
            "cursor": 0,
            "limit": 10,
        }),
        rmcp::model::NumberOrString::String("same-task-read-request-id".into()),
        task_only_headers,
    )
    .await
    .expect("read task-only sealed payload");
    let unknown_task = server
        .invoke_with_strategy_task_id::<_, Value>(
            "read_task_execution_snapshot",
            strategy_id,
            json!({}),
            Some("synthetic-unknown-task".into()),
            None,
            Some(task_id),
        )
        .await
        .map(|_| ())
        .map_err(|error| error.message.to_string());
    let other_strategy = server
        .invoke::<_, Value>(
            "read_task_execution_snapshot",
            other_strategy_id,
            json!({}),
            task_only_id.clone(),
            None,
        )
        .await
        .map(|_| ())
        .map_err(|error| error.message.to_string());
    strategy_task::ActiveModel {
        task_id: Set(task_id),
        deadline_at: Set((Utc::now() - chrono::Duration::seconds(1)).fixed_offset()),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("expire synthetic task deadline");
    let expired_task = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({
                "payload_id": payload.id,
                "pointer": "/value/items",
                "cursor": 0,
                "limit": 10,
            }),
            task_only_id,
            None,
        )
        .await
        .map(|_| ())
        .map_err(|error| error.message.to_string());
    let mut normalized_page = payload_page;
    normalized_page["payload_id"] = json!("<payload-id>");

    assert_eq!(
        (
            snapshot_read,
            normalized_page,
            unknown_task,
            other_strategy,
            expired_task,
        ),
        (
            json!({
                "definition": snapshot.definition,
                "digest": snapshot.digest,
            }),
            json!({
                "payload_id": "<payload-id>",
                "pointer": "/value/items",
                "data": [{"key": "synthetic"}],
                "cursor": 0,
                "next_cursor": null,
                "total_count": 1,
                "digest": payload.digest,
            }),
            Err("execution does not match an active task".into()),
            Err("execution does not match an active task".into()),
            Err("execution does not match an active task".into()),
        ),
    );
}

#[backend_test_macros::database_test]
async fn operation_mcp_call_consumes_the_task_tool_budget(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let task = tests_common::create_test_execution_task(
        &db,
        strategy_id,
        indoc! {"
            execution_limits:
              max_tool_calls: 0
            phases: []
        "},
    )
    .await;
    let execution_id = format!("{}:{}", task.a2a_task_id, Uuid::from_u128(712));
    let result = tests_common::build_server(db)
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            json!({
                "name": "synthetic-operation",
                "args": {},
                "payload_inputs": [],
            }),
            Some(execution_id),
            None,
        )
        .await
        .map(|_| ())
        .map_err(|error| error.message.to_string());

    assert_eq!(
        result,
        Err("execution budget limit was exceeded for __task_total".into()),
    );
}

#[backend_test_macros::database_test]
async fn mcp_operation_materializes_selected_payload_as_an_input_file(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (_, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let execution_step_id = Uuid::from_u128(707);
    let indicator_id = Uuid::from_u128(708);
    custom_indicator::ActiveModel {
        indicator_id: Set(indicator_id),
        name: Set("sample-operation".into()),
        scope: Set("strategy".into()),
        strategy_id: Set(Some(strategy_id)),
        code: Set("synthetic source".into()),
        input_schema: Set(json!({
            "type": "object",
            "properties": {"items": {"type": "object"}},
            "required": ["items"],
            "additionalProperties": false,
        })),
        output_schema: Set(json!({"type": "object"})),
        description: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(&db)
    .await
    .expect("insert synthetic operation");

    let base_server = tests_common::build_server(db.clone());
    let provider: SharedDatasetProvider = Arc::new(SyntheticDatasetProvider {
        items: vec![
            json!({"value": 11}),
            json!({"value": 22}),
            json!({"value": 33}),
        ],
        mode: ProviderMode::Stable,
    });
    let executor = Arc::new(FakeKataExecutor::new());
    executor
        .set_response(Ok(ExecResult {
            stdout: "{\"ok\":true}".into(),
            stderr: String::new(),
            exit_code: 0,
            files: Vec::new(),
        }))
        .await;
    let server = StrategyServer::new(
        base_server
            .entrypoint_server()
            .clone()
            .with_dataset_providers(BTreeMap::from([(
                "synthetic-provider".to_string(),
                provider,
            )])),
        db.clone(),
    )
    .with_kata_executor(Some(executor.clone()));
    let execution_id = format!("{a2a_task_id}:{execution_step_id}");
    let captured: Value = server
        .invoke::<_, Value>(
            "capture_dataset",
            strategy_id,
            json!({
                "provider": "synthetic-provider",
                "params": {"request_tag": "sample"},
                "page_size": 2,
            }),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("capture synthetic dataset")
        .into_value();
    let payload_id = Uuid::parse_str(
        captured["payload"]["id"]
            .as_str()
            .expect("captured payload ID"),
    )
    .expect("valid captured payload ID");
    server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            json!({
                "name": "sample-operation",
                "args": {},
                "payload_inputs": [{
                    "field": "items",
                    "payload_id": payload_id,
                    "pointer": "",
                    "cursor": 1,
                    "limit": 1,
                }],
                "timeout_secs": null,
                "max_output_bytes": null,
            }),
            Some(execution_id),
            None,
        )
        .await
        .expect("execute operation with selected payload input");

    let request = executor.requests.lock().await[0].clone();
    let stdin: Value = serde_json::from_str(request.stdin.as_deref().expect("operation stdin"))
        .expect("operation input JSON");
    let actual = (stdin, request.input_files);

    assert_eq!(
        actual,
        (
            json!({
                "args": {
                    "items": {
                        "path": "inputs/input-0.json",
                        "payload_id": payload_id,
                        "pointer": "",
                        "digest": DATASET_DIGEST,
                    },
                },
                "payload_inputs": [{
                    "field": "items",
                    "payload_id": payload_id,
                    "pointer": "",
                    "cursor": 1,
                    "limit": 1,
                    "digest": DATASET_DIGEST,
                    "path": "inputs/input-0.json",
                }],
            }),
            Some(vec![ExecFile {
                path: "inputs/input-0.json".into(),
                bytes: b"[{\"value\":22}]".to_vec(),
            }]),
        ),
    );
}

fn normalize_payload_ref(mut payload: PayloadRef) -> PayloadRef {
    payload.id = Uuid::nil();
    payload.task_id = Uuid::nil();
    payload
}

fn normalize_page(mut page: PayloadPage) -> PayloadPage {
    page.payload_id = Uuid::nil();
    page
}

#[backend_test_macros::database_test]
async fn synthetic_provider_payload_is_read_in_stable_bounded_pages(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let execution_step_id = Uuid::from_u128(701);
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone())
            .strategy_executions();
    let access = use_cases
        .authorize_execution(
            StrategyScope::from(strategy_id),
            &a2a_task_id,
            Some(execution_step_id),
        )
        .await
        .expect("authorize synthetic task");
    let staging_payload_id = use_cases
        .begin_payload(
            &access,
            PayloadDefinition {
                kind: PayloadKind::Output,
                source: "synthetic-output".into(),
                source_revision: Some(DATASET_REVISION.into()),
                source_locator: None,
                source_metadata: json!({}),
                schema: json!({"type": "array", "items": {"type": "object"}}),
            },
            true,
        )
        .await
        .expect("begin staging payload");
    let staging_read = use_cases
        .read_payload(
            &access,
            ReadPayloadQuery {
                payload_id: staging_payload_id,
                pointer: String::new(),
                cursor: 0,
                limit: 1,
            },
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string());
    let provider = SyntheticDatasetProvider {
        items: vec![
            json!({"value": 11}),
            json!({"value": 22}),
            json!({"value": 33}),
        ],
        mode: ProviderMode::Stable,
    };
    let payload_ref = use_cases
        .capture_dataset(
            StrategyScope::from(strategy_id),
            &access,
            "synthetic-provider".into(),
            &provider,
            json!({"request_tag": "sample"}),
            DatasetCaptureOptions {
                page_size: 2,
                cancellation: &tokio_util::sync::CancellationToken::new(),
            },
        )
        .await
        .expect("capture synthetic dataset");
    let captured_payload = task_execution_payload::Entity::find_by_id(payload_ref.id)
        .one(&db)
        .await
        .expect("query captured payload")
        .expect("captured payload exists");
    let mut capture_metadata = serde_json::to_value(
        captured_payload
            .capture_metadata
            .expect("capture metadata exists"),
    )
    .expect("serialize capture metadata");
    capture_metadata["captured_at"] = json!("<captured_at>");
    let first_query = ReadPayloadQuery {
        payload_id: payload_ref.id,
        pointer: String::new(),
        cursor: 0,
        limit: 2,
    };
    let first = use_cases
        .read_payload(&access, first_query.clone())
        .await
        .expect("read first page");
    let first_again = use_cases
        .read_payload(&access, first_query)
        .await
        .expect("reread first page");
    let second = use_cases
        .read_payload(
            &access,
            ReadPayloadQuery {
                payload_id: payload_ref.id,
                pointer: String::new(),
                cursor: 2,
                limit: 2,
            },
        )
        .await
        .expect("read second page");
    let (_, other_a2a_task_id) = create_running_task(&db, strategy_id).await;
    let other_access = use_cases
        .authorize_execution(StrategyScope::from(strategy_id), &other_a2a_task_id, None)
        .await
        .expect("authorize another task");
    let cross_task_read = use_cases
        .read_payload(
            &other_access,
            ReadPayloadQuery {
                payload_id: payload_ref.id,
                pointer: String::new(),
                cursor: 0,
                limit: 1,
            },
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string());

    assert_eq!(
        (
            task_id,
            staging_read,
            cross_task_read,
            normalize_payload_ref(payload_ref),
            capture_metadata,
            normalize_page(first),
            normalize_page(first_again),
            normalize_page(second),
        ),
        (
            task_id,
            Err("payload is not sealed".to_string()),
            Err("payload was not found in this task".to_string()),
            PayloadRef {
                id: Uuid::nil(),
                task_id: Uuid::nil(),
                kind: PayloadKind::Input,
                digest: DATASET_DIGEST.into(),
                item_count: 3,
                byte_count: 40,
                source_revision: Some(DATASET_REVISION.into()),
                source_locator: Some(json!({"page_key": "sample-page"})),
                source_metadata: json!({"provider_tag": "synthetic"}),
            },
            json!({
                "captured_at": "<captured_at>",
                "expected_count": 3,
                "received_count": 3,
                "missing_count": 0,
                "unretrieved_count": 0,
            }),
            PayloadPage {
                payload_id: Uuid::nil(),
                pointer: String::new(),
                data: json!([{"value": 11}, {"value": 22}]),
                cursor: 0,
                next_cursor: Some(2),
                total_count: 3,
                digest: DATASET_DIGEST.into(),
            },
            PayloadPage {
                payload_id: Uuid::nil(),
                pointer: String::new(),
                data: json!([{"value": 11}, {"value": 22}]),
                cursor: 0,
                next_cursor: Some(2),
                total_count: 3,
                digest: DATASET_DIGEST.into(),
            },
            PayloadPage {
                payload_id: Uuid::nil(),
                pointer: String::new(),
                data: json!([{"value": 33}]),
                cursor: 2,
                next_cursor: None,
                total_count: 3,
                digest: DATASET_DIGEST.into(),
            },
        ),
    );
}

#[backend_test_macros::database_test]
async fn mcp_capture_dataset_uses_registered_provider_and_returns_sealed_pages(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let execution_step_id = Uuid::from_u128(704);
    let base_server = tests_common::build_server(db.clone());
    let provider: SharedDatasetProvider = Arc::new(SyntheticDatasetProvider {
        items: vec![
            json!({"value": 11}),
            json!({"value": 22}),
            json!({"value": 33}),
        ],
        mode: ProviderMode::Stable,
    });
    let server = StrategyServer::new(
        base_server
            .entrypoint_server()
            .clone()
            .with_dataset_providers(BTreeMap::from([(
                "synthetic-provider".to_string(),
                provider,
            )])),
        db.clone(),
    );
    let execution_id = format!("{a2a_task_id}:{execution_step_id}");
    let captured: Value = server
        .invoke::<_, Value>(
            "capture_dataset",
            strategy_id,
            json!({
                "provider": "synthetic-provider",
                "params": {"request_tag": "sample"},
                "page_size": 2,
            }),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("capture dataset through MCP")
        .into_value();
    let payload_id = Uuid::parse_str(
        captured["payload"]["id"]
            .as_str()
            .expect("captured payload ID"),
    )
    .expect("valid captured payload ID");
    let persisted = task_execution_payload::Entity::find_by_id(payload_id)
        .one(&db)
        .await
        .expect("query captured payload")
        .expect("captured payload exists");
    let mut returned_payload = captured["payload"].clone();
    let returned_task_matches = returned_payload["task_id"] == json!(task_id);
    returned_payload["id"] = json!("<payload-id>");
    returned_payload["task_id"] = json!("<task-id>");
    let mut stored_page_one: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({
                "payload_id": payload_id,
                "pointer": "",
                "cursor": 0,
                "limit": 2,
            }),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("read first captured page through MCP")
        .into_value();
    stored_page_one["payload_id"] = json!("<payload-id>");
    let mut stored_page_two: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({
                "payload_id": payload_id,
                "pointer": "",
                "cursor": 2,
                "limit": 2,
            }),
            Some(execution_id),
            None,
        )
        .await
        .expect("read second captured page through MCP")
        .into_value();
    stored_page_two["payload_id"] = json!("<payload-id>");

    assert_eq!(
        (
            returned_task_matches,
            returned_payload,
            persisted.status,
            stored_page_one,
            stored_page_two,
        ),
        (
            true,
            json!({
                "id": "<payload-id>",
                "task_id": "<task-id>",
                "kind": "input",
                "digest": DATASET_DIGEST,
                "item_count": 3,
                "byte_count": 40,
                "source_revision": DATASET_REVISION,
                "source_locator": {"page_key": "sample-page"},
                "source_metadata": {"provider_tag": "synthetic"},
            }),
            "sealed".to_string(),
            json!({
                "payload_id": "<payload-id>",
                "pointer": "",
                "data": [{"value": 11}, {"value": 22}],
                "cursor": 0,
                "next_cursor": 2,
                "total_count": 3,
                "digest": DATASET_DIGEST,
            }),
            json!({
                "payload_id": "<payload-id>",
                "pointer": "",
                "data": [{"value": 33}],
                "cursor": 2,
                "next_cursor": null,
                "total_count": 3,
                "digest": DATASET_DIGEST,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn incomplete_provider_capture_cannot_be_read(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone())
            .strategy_executions();
    let mut outcomes = Vec::new();
    for mode in [
        ProviderMode::ChangeRevision,
        ProviderMode::ExceedPageLimit,
        ProviderMode::Truncated,
        ProviderMode::MismatchedCompletionCount,
        ProviderMode::MoreWithoutCursor,
    ] {
        let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
        let access = use_cases
            .authorize_execution(StrategyScope::from(strategy_id), &a2a_task_id, None)
            .await
            .expect("authorize synthetic task");
        let provider = SyntheticDatasetProvider {
            items: vec![json!({"value": 11}), json!({"value": 22})],
            mode,
        };
        let capture = use_cases
            .capture_dataset(
                StrategyScope::from(strategy_id),
                &access,
                "synthetic-provider".into(),
                &provider,
                json!({}),
                DatasetCaptureOptions {
                    page_size: 1,
                    cancellation: &tokio_util::sync::CancellationToken::new(),
                },
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        let stored = task_execution_payload::Entity::find()
            .filter(task_execution_payload::Column::TaskId.eq(task_id))
            .one(&db)
            .await
            .expect("query incomplete payload")
            .expect("incomplete payload row exists");
        let mut capture_metadata = serde_json::to_value(
            stored
                .capture_metadata
                .clone()
                .expect("incomplete capture metadata exists"),
        )
        .expect("serialize incomplete capture metadata");
        capture_metadata["captured_at"] = json!("<captured_at>");
        let read = use_cases
            .read_payload(
                &access,
                ReadPayloadQuery {
                    payload_id: stored.id,
                    pointer: String::new(),
                    cursor: 0,
                    limit: 1,
                },
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        outcomes.push((
            capture,
            stored.status,
            stored.incomplete_reason,
            capture_metadata,
            read,
        ));
    }
    assert_eq!(
        outcomes,
        vec![
            (
                Err("payload source revision changed between pages".to_string()),
                "incomplete".to_string(),
                Some("source_revision_changed".to_string()),
                json!({
                    "captured_at": "<captured_at>", "expected_count": 2,
                    "received_count": 1, "missing_count": null, "unretrieved_count": null,
                }),
                Err("payload is not sealed".to_string()),
            ),
            (
                Err("dataset provider returned an invalid cursor".to_string()),
                "incomplete".to_string(),
                Some("provider_page_size_exceeded".to_string()),
                json!({
                    "captured_at": "<captured_at>", "expected_count": 2,
                    "received_count": 2, "missing_count": null, "unretrieved_count": null,
                }),
                Err("payload is not sealed".to_string()),
            ),
            (
                Err("dataset provider reported an incomplete capture".to_string()),
                "incomplete".to_string(),
                Some("provider_incomplete".to_string()),
                json!({
                    "captured_at": "<captured_at>", "expected_count": 2,
                    "received_count": 1, "missing_count": 0, "unretrieved_count": 1,
                }),
                Err("payload is not sealed".to_string()),
            ),
            (
                Err("dataset provider returned inconsistent completion counts".to_string()),
                "incomplete".to_string(),
                Some("invalid_provider_completion".to_string()),
                json!({
                    "captured_at": "<captured_at>", "expected_count": 2,
                    "received_count": 2, "missing_count": null, "unretrieved_count": null,
                }),
                Err("payload is not sealed".to_string()),
            ),
            (
                Err("dataset provider reported an incomplete capture".to_string()),
                "incomplete".to_string(),
                Some("provider_completion_unconfirmed".to_string()),
                json!({
                    "captured_at": "<captured_at>", "expected_count": 2,
                    "received_count": 1, "missing_count": null, "unretrieved_count": null,
                }),
                Err("payload is not sealed".to_string()),
            ),
        ],
    );
}

#[backend_test_macros::database_test]
async fn unknown_provider_total_is_persisted_without_inventing_a_count(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone())
            .strategy_executions();
    let access = use_cases
        .authorize_execution(StrategyScope::from(strategy_id), &a2a_task_id, None)
        .await
        .expect("authorize synthetic task");
    let provider = SyntheticDatasetProvider {
        items: vec![json!({"value": 11}), json!({"value": 22})],
        mode: ProviderMode::UnknownExpectedCount,
    };
    let payload = use_cases
        .capture_dataset(
            StrategyScope::from(strategy_id),
            &access,
            "synthetic-provider".into(),
            &provider,
            json!({}),
            DatasetCaptureOptions {
                page_size: 1,
                cancellation: &tokio_util::sync::CancellationToken::new(),
            },
        )
        .await
        .expect("capture dataset with unknown total");
    let stored = task_execution_payload::Entity::find_by_id(payload.id)
        .filter(task_execution_payload::Column::TaskId.eq(task_id))
        .one(&db)
        .await
        .expect("query captured payload")
        .expect("captured payload row exists");
    let mut capture_metadata =
        serde_json::to_value(stored.capture_metadata.expect("capture metadata exists"))
            .expect("serialize capture metadata");
    capture_metadata["captured_at"] = json!("<captured_at>");

    assert_eq!(
        (stored.status, capture_metadata),
        (
            "sealed".to_string(),
            json!({
                "captured_at": "<captured_at>",
                "expected_count": null,
                "received_count": 2,
                "missing_count": null,
                "unretrieved_count": 0,
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn cancelled_dataset_capture_marks_its_payload_incomplete(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone())
            .strategy_executions();
    let access = use_cases
        .authorize_execution(StrategyScope::from(strategy_id), &a2a_task_id, None)
        .await
        .expect("authorize synthetic task");
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = WaitingDatasetProvider {
        started: Arc::clone(&started),
        wait_at: DatasetWaitStage::ReadPage,
    };
    let cancellation = tokio_util::sync::CancellationToken::new();
    let capture_cancellation = cancellation.clone();
    let capture = tokio::spawn(async move {
        use_cases
            .capture_dataset(
                StrategyScope::from(strategy_id),
                &access,
                "synthetic-provider".into(),
                &provider,
                json!({}),
                DatasetCaptureOptions {
                    page_size: 10,
                    cancellation: &capture_cancellation,
                },
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    });

    tokio::time::timeout(Duration::from_secs(5), started.notified())
        .await
        .expect("dataset page read starts");
    cancellation.cancel();
    let capture_result = capture.await.expect("join cancelled capture");
    let stored = task_execution_payload::Entity::find()
        .filter(task_execution_payload::Column::TaskId.eq(task_id))
        .one(&db)
        .await
        .expect("query cancelled payload")
        .expect("cancelled payload row exists");

    assert_eq!(
        (capture_result, stored.status, stored.incomplete_reason),
        (
            Err("dataset capture was cancelled".to_string()),
            "incomplete".to_string(),
            Some("request_cancelled".to_string()),
        ),
    );
}

#[backend_test_macros::database_test]
async fn cancelled_dataset_open_does_not_create_a_payload(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let use_cases =
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone())
            .strategy_executions();
    let access = use_cases
        .authorize_execution(StrategyScope::from(strategy_id), &a2a_task_id, None)
        .await
        .expect("authorize synthetic task");
    let started = Arc::new(tokio::sync::Notify::new());
    let provider = WaitingDatasetProvider {
        started: Arc::clone(&started),
        wait_at: DatasetWaitStage::Open,
    };
    let cancellation = tokio_util::sync::CancellationToken::new();
    let capture_cancellation = cancellation.clone();
    let capture = tokio::spawn(async move {
        use_cases
            .capture_dataset(
                StrategyScope::from(strategy_id),
                &access,
                "synthetic-provider".into(),
                &provider,
                json!({}),
                DatasetCaptureOptions {
                    page_size: 10,
                    cancellation: &capture_cancellation,
                },
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    });

    tokio::time::timeout(Duration::from_secs(5), started.notified())
        .await
        .expect("dataset provider open starts");
    cancellation.cancel();
    let capture_result = capture.await.expect("join cancelled dataset open");
    let stored = task_execution_payload::Entity::find()
        .filter(task_execution_payload::Column::TaskId.eq(task_id))
        .one(&db)
        .await
        .expect("query payload created during cancelled open");

    assert_eq!(
        (capture_result, stored.is_none()),
        (Err("dataset capture was cancelled".to_string()), true),
    );
}

#[backend_test_macros::database_test]
async fn execute_operation_seals_payloads_and_preserves_numeric_eval_shape(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (task_id, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let execution_step_id = Uuid::from_u128(702);
    let indicator_id = Uuid::from_u128(703);
    custom_indicator::ActiveModel {
        indicator_id: Set(indicator_id),
        name: Set("sample-operation".into()),
        scope: Set("strategy".into()),
        strategy_id: Set(Some(strategy_id)),
        code: Set("synthetic source A".into()),
        input_schema: Set(json!({"type": "object"})),
        output_schema: Set(json!({
            "type": "array",
            "items": {"type": "object", "properties": {"score": {"type": "number"}},
                "required": ["score"], "additionalProperties": false},
        })),
        description: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(&db)
    .await
    .expect("insert synthetic operation");

    let executor = Arc::new(FakeKataExecutor::new());
    executor
        .set_response(Ok(ExecResult {
            stdout: "[{\"score\":12.5},{\"score\":20},{\"score\":30}]\n".into(),
            stderr: String::new(),
            exit_code: 0,
            files: vec![],
        }))
        .await;
    let shared_executor: SharedKataExecutor = executor.clone();
    let server = tests_common::build_server(db.clone()).with_kata_executor(Some(shared_executor));
    let execution_id = format!("{a2a_task_id}:{execution_step_id}");
    let params = json!({
        "name": "sample-operation",
        "args": {"sample": "value"},
        "numeric_pointers": {"result": "/value/1/score"},
        "timeout_secs": null,
        "max_output_bytes": null,
    });
    let first: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            params.clone(),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("execute saved operation")
        .into_value();
    let output_payload_id = Uuid::parse_str(
        first["output_payload"]["id"]
            .as_str()
            .expect("output payload ID"),
    )
    .expect("valid output payload ID");
    let input_payload_id = Uuid::parse_str(
        first["input_payload"]["id"]
            .as_str()
            .expect("input payload ID"),
    )
    .expect("valid input payload ID");
    let evaluation_id = Uuid::parse_str(first["evaluation_id"].as_str().expect("evaluation ID"))
        .expect("valid evaluation ID");
    custom_indicator::ActiveModel {
        indicator_id: sea_orm::ActiveValue::Unchanged(indicator_id),
        code: Set("synthetic source B".into()),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("update live definition after first use");
    executor
        .set_response(Ok(ExecResult {
            stdout: "[{\"score\":12.5},{\"score\":20},{\"score\":30}]\n".into(),
            stderr: String::new(),
            exit_code: 0,
            files: vec![],
        }))
        .await;
    let second: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            params.clone(),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("reuse task definition snapshot")
        .into_value();
    executor
        .set_response(Ok(ExecResult {
            stdout: "synthetic output".into(),
            stderr: "synthetic failure".into(),
            exit_code: 17,
            files: vec![],
        }))
        .await;
    let failure: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            params,
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("persist nonzero operation result")
        .into_value();
    let failure_payload_id = Uuid::parse_str(
        failure["output_payload"]["id"]
            .as_str()
            .expect("failed output payload ID"),
    )
    .expect("valid failed output payload ID");
    let failure_page: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({"payload_id": failure_payload_id, "pointer": "", "cursor": 0, "limit": 1}),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("read failed operation payload")
        .into_value();
    let failure_evaluations = custom_indicator_evaluation::Entity::find()
        .filter(custom_indicator_evaluation::Column::PayloadId.eq(failure_payload_id))
        .all(&db)
        .await
        .expect("query evaluations linked to failed output");

    let first_query =
        json!({"payload_id": output_payload_id, "pointer": "/value", "cursor": 0, "limit": 1});
    let first_page: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            first_query.clone(),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("read output payload first page")
        .into_value();
    let first_page_again: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            first_query,
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("reread output payload first page")
        .into_value();
    let second_page: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({"payload_id": output_payload_id, "pointer": "/value", "cursor": 1, "limit": 1}),
            Some(execution_id),
            None,
        )
        .await
        .expect("read output payload second page")
        .into_value();
    let input_page: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({"payload_id": input_payload_id, "pointer": "/args", "cursor": 0, "limit": 1}),
            Some(format!("{a2a_task_id}:{execution_step_id}")),
            None,
        )
        .await
        .expect("read sealed input payload")
        .into_value();
    let evaluation = custom_indicator_evaluation::Entity::find_by_id(evaluation_id)
        .one(&db)
        .await
        .expect("query numeric evaluation")
        .expect("numeric evaluation exists");
    let evaluation_token = format!("[[eval:{evaluation_id}:result]]");
    let note = server
        .write_note(
            strategy_id,
            None,
            super::dto::WriteNoteParams {
                note_id: None,
                title: Some("synthetic note".into()),
                body_md: Some(evaluation_token.clone()),
                kind: None,
                change_reason: None,
                frontmatter_json: None,
                graphs: None,
            },
        )
        .await
        .expect("write note citing operation evaluation")
        .into_value();
    let note_version = crate::testing::find_current_note_version(&db, note.note_id)
        .await
        .expect("query current note version")
        .expect("current note version exists");
    let note_reference_values = json!({
        (evaluation_token): {"value": 20},
    });
    let definition_snapshots = task_execution_definition_snapshot::Entity::find()
        .filter(task_execution_definition_snapshot::Column::TaskId.eq(task_id))
        .filter(
            task_execution_definition_snapshot::Column::DefinitionKey
                .eq("custom_indicator:sample-operation"),
        )
        .all(&db)
        .await
        .expect("query definition snapshots");
    let definition_digest = definition_snapshots
        .first()
        .expect("pinned definition snapshot exists")
        .digest
        .clone();
    let requests: Vec<ExecRequest> = executor.requests.lock().await.clone();
    let first = normalize_operation(first);
    let second = normalize_operation(second);
    let failure = normalize_operation(failure);
    let first_page = normalize_mcp_page(first_page);
    let first_page_again = normalize_mcp_page(first_page_again);
    let second_page = normalize_mcp_page(second_page);
    let input_page = normalize_mcp_page(input_page);
    let failure_page = normalize_mcp_page(failure_page);
    let evaluation = normalize_evaluation(evaluation);
    let output_byte_count = serde_json::to_vec(&json!({
        "exit_code": 0,
        "stdout": "[{\"score\":12.5},{\"score\":20},{\"score\":30}]\n",
        "stderr": "",
        "value": [{"score": 12.5}, {"score": 20}, {"score": 30}],
    }))
    .expect("serialize expected output")
    .len() as u64;
    let failure_output = json!({
        "exit_code": 17,
        "stderr": "synthetic failure",
        "stdout": "synthetic output",
        "value": null,
    });
    let failure_byte_count = serde_json::to_vec(&failure_output)
        .expect("serialize expected failed output")
        .len() as u64;

    assert_eq!(
        (
            first,
            second,
            failure,
            first_page,
            first_page_again,
            second_page,
            input_page,
            failure_page,
            failure_evaluations,
            (
                evaluation,
                note_version.resolved_price_references_json,
                definition_snapshots.len(),
            ),
            requests,
        ),
        (
            json!({
                "input_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": OPERATION_INPUT_DIGEST, "item_count": 1, "byte_count": 27,
                    "source_revision": definition_digest,
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": definition_digest}},
                "output_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": OPERATION_OUTPUT_DIGEST, "item_count": 1, "byte_count": output_byte_count,
                    "source_revision": definition_digest,
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": definition_digest}},
                "evaluation_id": Uuid::nil(), "projection_error": null,
                "exit_code": 0, "stderr": "",
            }),
            json!({
                "input_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": OPERATION_INPUT_DIGEST, "item_count": 1, "byte_count": 27,
                    "source_revision": definition_digest,
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": definition_digest}},
                "output_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": OPERATION_OUTPUT_DIGEST, "item_count": 1, "byte_count": output_byte_count,
                    "source_revision": definition_digest,
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": definition_digest}},
                "evaluation_id": Uuid::nil(), "projection_error": null,
                "exit_code": 0, "stderr": "",
            }),
            json!({
                "input_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": OPERATION_INPUT_DIGEST, "item_count": 1, "byte_count": 27,
                    "source_revision": definition_digest,
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": definition_digest}},
                "output_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": OPERATION_FAILURE_DIGEST, "item_count": 1, "byte_count": failure_byte_count,
                    "source_revision": definition_digest,
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": definition_digest}},
                "evaluation_id": null, "projection_error": null,
                "exit_code": 17, "stderr": "synthetic failure",
            }),
            json!({"payload_id": Uuid::nil(), "pointer": "/value", "data": [{"score": 12.5}],
                "cursor": 0, "next_cursor": 1, "total_count": 3, "digest": OPERATION_OUTPUT_DIGEST}),
            json!({"payload_id": Uuid::nil(), "pointer": "/value", "data": [{"score": 12.5}],
                "cursor": 0, "next_cursor": 1, "total_count": 3, "digest": OPERATION_OUTPUT_DIGEST}),
            json!({"payload_id": Uuid::nil(), "pointer": "/value", "data": [{"score": 20}],
                "cursor": 1, "next_cursor": 2, "total_count": 3, "digest": OPERATION_OUTPUT_DIGEST}),
            json!({"payload_id": Uuid::nil(), "pointer": "/args", "data": {"sample": "value"},
                "cursor": 0, "next_cursor": null, "total_count": 1, "digest": OPERATION_INPUT_DIGEST}),
            json!({"payload_id": Uuid::nil(), "pointer": "", "data": failure_output,
                "cursor": 0, "next_cursor": null, "total_count": 1, "digest": OPERATION_FAILURE_DIGEST}),
            Vec::<custom_indicator_evaluation::Model>::new(),
            (
                custom_indicator_evaluation::Model {
                    id: Uuid::nil(),
                    indicator_id,
                    indicator_name: "sample-operation".into(),
                    code: "synthetic source A".into(),
                    args: json!({"payload_id": Uuid::nil(), "pointers": {"result": "/value/1/score"},
                        "args": {"sample": "value"}}),
                    output: json!({"result": 20}),
                    evaluated_at: tests_common::ts_sentinel(),
                    payload_id: Some(Uuid::nil()),
                    payload_pointers: Some(json!({"result": "/value/1/score"})),
                    execution_step_id: Some(execution_step_id),
                },
                note_reference_values,
                1,
            ),
            vec![
                ExecRequest {
                    code: "synthetic source A".into(),
                    stdin: Some(r#"{"args":{"sample":"value"}}"#.into()),
                    timeout: None,
                    max_output_bytes: Some(1024 * 1024),
                    input_files: None,
                },
                ExecRequest {
                    code: "synthetic source A".into(),
                    stdin: Some(r#"{"args":{"sample":"value"}}"#.into()),
                    timeout: None,
                    max_output_bytes: Some(1024 * 1024),
                    input_files: None,
                },
                ExecRequest {
                    code: "synthetic source A".into(),
                    stdin: Some(r#"{"args":{"sample":"value"}}"#.into()),
                    timeout: None,
                    max_output_bytes: Some(1024 * 1024),
                    input_files: None,
                },
            ],
        ),
    );
}

#[backend_test_macros::database_test]
async fn operation_projection_is_optional_and_cannot_cite_wrapper_metadata(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (_, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let execution_step_id = Uuid::from_u128(704);
    for (indicator_id, name, code, output_schema) in [
        (
            Uuid::from_u128(705),
            "sample-number-operation",
            "synthetic numeric source",
            json!({"type": "number"}),
        ),
        (
            Uuid::from_u128(706),
            "sample-text-operation",
            "synthetic text source",
            json!({"type": "string"}),
        ),
    ] {
        custom_indicator::ActiveModel {
            indicator_id: Set(indicator_id),
            name: Set(name.into()),
            scope: Set("strategy".into()),
            strategy_id: Set(Some(strategy_id)),
            code: Set(code.into()),
            input_schema: Set(json!({"type": "object"})),
            output_schema: Set(output_schema),
            description: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await
        .expect("insert synthetic operation definition");
    }

    let executor = Arc::new(FakeKataExecutor::new());
    let shared_executor: SharedKataExecutor = executor.clone();
    let server = tests_common::build_server(db.clone()).with_kata_executor(Some(shared_executor));
    let execution_id = format!("{a2a_task_id}:{execution_step_id}");
    executor
        .set_response(Ok(ExecResult {
            stdout: "13\n".into(),
            stderr: String::new(),
            exit_code: 0,
            files: vec![],
        }))
        .await;
    let wrapper_projection: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            json!({
                "name": "sample-number-operation",
                "args": {},
                "numeric_pointers": {"result": "/exit_code"},
            }),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("operation succeeds when wrapper metadata projection is rejected")
        .into_value();
    let wrapper_payload_id = Uuid::parse_str(
        wrapper_projection["output_payload"]["id"]
            .as_str()
            .expect("wrapper output payload ID"),
    )
    .expect("valid wrapper output payload ID");
    let wrapper_payload: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({"payload_id": wrapper_payload_id, "pointer": "", "cursor": 0, "limit": 1}),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("read output after rejected projection")
        .into_value();

    executor
        .set_response(Ok(ExecResult {
            stdout: "\"fictional result\"\n".into(),
            stderr: String::new(),
            exit_code: 0,
            files: vec![],
        }))
        .await;
    let text_output: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            json!({
                "name": "sample-text-operation",
                "args": {},
            }),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("non-numeric operation succeeds without a numeric projection")
        .into_value();
    let text_payload_id = Uuid::parse_str(
        text_output["output_payload"]["id"]
            .as_str()
            .expect("text output payload ID"),
    )
    .expect("valid text output payload ID");
    let text_payload: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({"payload_id": text_payload_id, "pointer": "/value", "cursor": 0, "limit": 1}),
            Some(execution_id),
            None,
        )
        .await
        .expect("read non-numeric output payload")
        .into_value();

    let wrapper_output = json!({
        "exit_code": 0,
        "stderr": "",
        "stdout": "13\n",
        "value": 13,
    });
    let text_output_root = json!({
        "exit_code": 0,
        "stderr": "",
        "stdout": "\"fictional result\"\n",
        "value": "fictional result",
    });
    assert_eq!(
        (
            normalize_operation_payload_identity(wrapper_projection),
            normalize_payload_page_identity(wrapper_payload),
            normalize_operation_payload_identity(text_output),
            normalize_payload_page_identity(text_payload),
        ),
        (
            json!({
                "input_payload": {
                    "id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": "<digest>", "item_count": 1, "byte_count": 11,
                    "source_revision": "<revision>",
                    "source_locator": {"definition_key": "custom_indicator:sample-number-operation"},
                    "source_metadata": {"definition_digest": "<revision>"},
                },
                "output_payload": {
                    "id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": "<digest>", "item_count": 1,
                    "byte_count": serde_json::to_vec(&wrapper_output).expect("serialize wrapper output").len(),
                    "source_revision": "<revision>",
                    "source_locator": {"definition_key": "custom_indicator:sample-number-operation"},
                    "source_metadata": {"definition_digest": "<revision>"},
                },
                "evaluation_id": null,
                "projection_error": "invalid_projection",
                "exit_code": 0,
                "stderr": "",
            }),
            json!({
                "payload_id": Uuid::nil(), "pointer": "", "data": wrapper_output,
                "cursor": 0, "next_cursor": null, "total_count": 1, "digest": "<digest>",
            }),
            json!({
                "input_payload": {
                    "id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": "<digest>", "item_count": 1, "byte_count": 11,
                    "source_revision": "<revision>",
                    "source_locator": {"definition_key": "custom_indicator:sample-text-operation"},
                    "source_metadata": {"definition_digest": "<revision>"},
                },
                "output_payload": {
                    "id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": "<digest>", "item_count": 1,
                    "byte_count": serde_json::to_vec(&text_output_root).expect("serialize text output").len(),
                    "source_revision": "<revision>",
                    "source_locator": {"definition_key": "custom_indicator:sample-text-operation"},
                    "source_metadata": {"definition_digest": "<revision>"},
                },
                "evaluation_id": null,
                "projection_error": null,
                "exit_code": 0,
                "stderr": "",
            }),
            json!({
                "payload_id": Uuid::nil(), "pointer": "/value", "data": "fictional result",
                "cursor": 0, "next_cursor": null, "total_count": 1, "digest": "<digest>",
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn projection_recording_failure_keeps_the_saved_output_reference(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = tests_common::insert_strategy(&db, "sample-strategy").await;
    let (_, a2a_task_id) = create_running_task(&db, strategy_id).await;
    let indicator_id = Uuid::from_u128(707);
    custom_indicator::ActiveModel {
        indicator_id: Set(indicator_id),
        name: Set("sample-operation".into()),
        scope: Set("strategy".into()),
        strategy_id: Set(Some(strategy_id)),
        code: Set("synthetic source".into()),
        input_schema: Set(json!({"type": "object"})),
        output_schema: Set(json!({"type": "number"})),
        description: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(&db)
    .await
    .expect("insert synthetic operation definition");
    db.execute_unprepared(
        "CREATE FUNCTION skip_numeric_evaluation_for_test() RETURNS trigger \
         LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$",
    )
    .await
    .expect("create evaluation skip function");
    db.execute_unprepared(
        "CREATE TRIGGER skip_numeric_evaluation_for_test BEFORE INSERT \
         ON custom_indicator_evaluation FOR EACH ROW \
         EXECUTE FUNCTION skip_numeric_evaluation_for_test()",
    )
    .await
    .expect("create evaluation skip trigger");

    let executor = Arc::new(FakeKataExecutor::new());
    executor
        .set_response(Ok(ExecResult {
            stdout: "13\n".into(),
            stderr: String::new(),
            exit_code: 0,
            files: vec![],
        }))
        .await;
    let server = tests_common::build_server(db.clone())
        .with_kata_executor(Some(executor.clone() as SharedKataExecutor));
    let execution_id = format!("{a2a_task_id}:{}", Uuid::from_u128(708));
    let operation: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            json!({
                "name": "sample-operation",
                "args": {},
                "numeric_pointers": {"result": "/value"},
            }),
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("saved operation succeeds when projection recording fails")
        .into_value();
    let output_payload_id = Uuid::parse_str(
        operation["output_payload"]["id"]
            .as_str()
            .expect("output payload ID"),
    )
    .expect("valid output payload ID");
    let output_page: Value = server
        .invoke::<_, Value>(
            "read_execution_payload",
            strategy_id,
            json!({"payload_id": output_payload_id, "pointer": "", "cursor": 0, "limit": 1}),
            Some(execution_id),
            None,
        )
        .await
        .expect("read saved output after projection recording fails")
        .into_value();
    let evaluations = custom_indicator_evaluation::Entity::find()
        .filter(custom_indicator_evaluation::Column::IndicatorId.eq(indicator_id))
        .all(&db)
        .await
        .expect("query failed projection evaluations");
    let output = json!({
        "exit_code": 0,
        "stderr": "",
        "stdout": "13\n",
        "value": 13,
    });

    assert_eq!(
        (
            normalize_operation_payload_identity(operation),
            normalize_payload_page_identity(output_page),
            evaluations,
        ),
        (
            json!({
                "input_payload": {
                    "id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": "<digest>", "item_count": 1, "byte_count": 11,
                    "source_revision": "<revision>",
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": "<revision>"},
                },
                "output_payload": {
                    "id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": "<digest>", "item_count": 1,
                    "byte_count": serde_json::to_vec(&output).expect("serialize output").len(),
                    "source_revision": "<revision>",
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": "<revision>"},
                },
                "evaluation_id": null,
                "projection_error": "recording_failed",
                "exit_code": 0,
                "stderr": "",
            }),
            json!({
                "payload_id": Uuid::nil(), "pointer": "", "data": output,
                "cursor": 0, "next_cursor": null, "total_count": 1, "digest": "<digest>",
            }),
            Vec::<custom_indicator_evaluation::Model>::new(),
        ),
    );
}

#[backend_test_macros::database_test]
async fn execution_scope_rejects_another_strategy_before_running_saved_code(
    db: gateway_postgres::DatabaseHandle,
) {
    let owner_strategy = tests_common::insert_strategy(&db, "owner-strategy").await;
    let other_strategy = tests_common::insert_strategy(&db, "other-strategy").await;
    let (_, a2a_task_id) = create_running_task(&db, owner_strategy).await;
    let executor = Arc::new(FakeKataExecutor::new());
    let shared_executor: SharedKataExecutor = executor.clone();
    let server = tests_common::build_server(db).with_kata_executor(Some(shared_executor));
    let result = server
        .invoke::<_, Value>(
            "execute_operation",
            other_strategy,
            json!({
                "name": "sample-operation",
                "args": {},
                "numeric_pointers": {"result": "/value"},
                "timeout_secs": null,
                "max_output_bytes": null,
            }),
            Some(format!("{a2a_task_id}:{}", Uuid::from_u128(704))),
            None,
        )
        .await
        .map(|_| ())
        .map_err(|error| error.to_string());
    let requests = executor.requests.lock().await.clone();

    assert_eq!(
        (result, requests),
        (
            Err("-32602: execution does not match an active task".to_string()),
            Vec::<ExecRequest>::new(),
        ),
    );
}

fn normalize_operation(mut operation: Value) -> Value {
    for kind in ["input_payload", "output_payload"] {
        operation[kind]["id"] = json!(Uuid::nil());
        operation[kind]["task_id"] = json!(Uuid::nil());
    }
    if operation["evaluation_id"].is_string() {
        operation["evaluation_id"] = json!(Uuid::nil());
    }
    operation
}

fn normalize_mcp_page(mut page: Value) -> Value {
    page["payload_id"] = json!(Uuid::nil());
    page
}

fn normalize_operation_payload_identity(mut operation: Value) -> Value {
    for kind in ["input_payload", "output_payload"] {
        operation[kind]["id"] = json!(Uuid::nil());
        operation[kind]["task_id"] = json!(Uuid::nil());
        operation[kind]["digest"] = json!("<digest>");
        operation[kind]["source_revision"] = json!("<revision>");
        operation[kind]["source_metadata"]["definition_digest"] = json!("<revision>");
    }
    operation
}

fn normalize_payload_page_identity(mut page: Value) -> Value {
    page["payload_id"] = json!(Uuid::nil());
    page["digest"] = json!("<digest>");
    page
}

fn normalize_evaluation(
    mut evaluation: custom_indicator_evaluation::Model,
) -> custom_indicator_evaluation::Model {
    evaluation.id = Uuid::nil();
    evaluation.args["payload_id"] = json!(Uuid::nil());
    evaluation.payload_id = Some(Uuid::nil());
    evaluation.evaluated_at = tests_common::ts_sentinel();
    evaluation
}
