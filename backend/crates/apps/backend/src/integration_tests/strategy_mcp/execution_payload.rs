use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use core_application::{
    kata_exec::{ExecRequest, ExecResult, FakeKataExecutor, SharedKataExecutor},
    strategy_execution::{
        DatasetPage, DatasetProvider, DatasetProviderError, DatasetSnapshot, PayloadDefinition,
        PayloadKind, PayloadPage, PayloadRef, ReadPayloadQuery,
    },
    strategy_scope::StrategyScope,
};
use gateway_postgres::entities::{
    custom_indicator, custom_indicator_evaluation, sea_orm_active_enums::StrategyTaskPhase,
    strategy_task, task_execution_definition_snapshot, task_execution_payload,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, EntityTrait, QueryFilter,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::tests_common;

const DATASET_REVISION: &str = "sample-revision-a";

#[derive(Clone, Copy)]
enum ProviderMode {
    Stable,
    ChangeRevision,
    ExceedPageLimit,
}

struct SyntheticDatasetProvider {
    items: Vec<Value>,
    mode: ProviderMode,
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
        Ok(DatasetPage {
            source_revision: source_revision.into(),
            items: self.items[start..end].to_vec(),
            next_cursor: (end < self.items.len()).then(|| end.to_string()),
        })
    }
}

async fn create_running_task(
    db: &gateway_postgres::DatabaseHandle,
    strategy_id: Uuid,
) -> (Uuid, String) {
    let task_id = crate::testing::insert_test_strategy_task(
        db,
        strategy_id,
        "synthetic task prompt",
        None,
        Utc::now().fixed_offset(),
    )
    .await;
    let a2a_task_id = format!("sample-task-{}", Uuid::new_v4());
    strategy_task::ActiveModel {
        task_id: Set(task_id),
        a2a_task_id: Set(Some(a2a_task_id.clone())),
        phase: Set(StrategyTaskPhase::Running),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("update synthetic task");
    (task_id, a2a_task_id)
}

fn normalize_payload_ref(mut payload: PayloadRef) -> PayloadRef {
    payload.id = Uuid::nil();
    payload.task_id = Uuid::nil();
    payload.digest = "<digest>".into();
    payload
}

fn normalize_page(mut page: PayloadPage) -> PayloadPage {
    page.payload_id = Uuid::nil();
    page.digest = "<digest>".into();
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
            2,
        )
        .await
        .expect("capture synthetic dataset");
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
                digest: "<digest>".into(),
                item_count: 3,
                byte_count: 40,
                source_revision: Some(DATASET_REVISION.into()),
                source_locator: Some(json!({"page_key": "sample-page"})),
                source_metadata: json!({"provider_tag": "synthetic"}),
            },
            PayloadPage {
                payload_id: Uuid::nil(),
                pointer: String::new(),
                data: json!([{"value": 11}, {"value": 22}]),
                cursor: 0,
                next_cursor: Some(2),
                total_count: 3,
                digest: "<digest>".into(),
            },
            PayloadPage {
                payload_id: Uuid::nil(),
                pointer: String::new(),
                data: json!([{"value": 11}, {"value": 22}]),
                cursor: 0,
                next_cursor: Some(2),
                total_count: 3,
                digest: "<digest>".into(),
            },
            PayloadPage {
                payload_id: Uuid::nil(),
                pointer: String::new(),
                data: json!([{"value": 33}]),
                cursor: 2,
                next_cursor: None,
                total_count: 3,
                digest: "<digest>".into(),
            },
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
    for mode in [ProviderMode::ChangeRevision, ProviderMode::ExceedPageLimit] {
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
                1,
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
        outcomes.push((capture, stored.status, stored.incomplete_reason, read));
    }
    assert_eq!(
        outcomes,
        vec![
            (
                Err("payload source revision changed between pages".to_string()),
                "incomplete".to_string(),
                Some("source_revision_changed".to_string()),
                Err("payload is not sealed".to_string()),
            ),
            (
                Err("dataset provider returned an invalid cursor".to_string()),
                "incomplete".to_string(),
                Some("provider_page_size_exceeded".to_string()),
                Err("payload is not sealed".to_string()),
            ),
        ],
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
        }))
        .await;
    let second: Value = server
        .invoke::<_, Value>(
            "execute_operation",
            strategy_id,
            params,
            Some(execution_id.clone()),
            None,
        )
        .await
        .expect("reuse task definition snapshot")
        .into_value();

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
    let definition_snapshots = task_execution_definition_snapshot::Entity::find()
        .filter(task_execution_definition_snapshot::Column::TaskId.eq(task_id))
        .all(&db)
        .await
        .expect("query definition snapshots");
    let mut requests: Vec<ExecRequest> = executor.requests.lock().await.clone();
    requests
        .iter_mut()
        .for_each(|request| request.code = "<pinned-code>".into());
    let first = normalize_operation(first);
    let second = normalize_operation(second);
    let first_page = normalize_mcp_page(first_page);
    let first_page_again = normalize_mcp_page(first_page_again);
    let second_page = normalize_mcp_page(second_page);
    let input_page = normalize_mcp_page(input_page);
    let evaluation = normalize_evaluation(evaluation);
    let output_byte_count = serde_json::to_vec(&json!({
        "exit_code": 0,
        "stdout": "[{\"score\":12.5},{\"score\":20},{\"score\":30}]\n",
        "stderr": "",
        "value": [{"score": 12.5}, {"score": 20}, {"score": 30}],
    }))
    .expect("serialize expected output")
    .len() as u64;

    assert_eq!(
        (
            first,
            second,
            first_page,
            first_page_again,
            second_page,
            input_page,
            evaluation,
            definition_snapshots.len(),
            requests,
        ),
        (
            json!({
                "input_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": "<digest>", "item_count": 1, "byte_count": 27,
                    "source_revision": "<definition-digest>",
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": "<definition-digest>"}},
                "output_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": "<digest>", "item_count": 1, "byte_count": output_byte_count,
                    "source_revision": "<definition-digest>",
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": "<definition-digest>"}},
                "evaluation_id": Uuid::nil(), "exit_code": 0, "stderr": "",
            }),
            json!({
                "input_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "input",
                    "digest": "<digest>", "item_count": 1, "byte_count": 27,
                    "source_revision": "<definition-digest>",
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": "<definition-digest>"}},
                "output_payload": {"id": Uuid::nil(), "task_id": Uuid::nil(), "kind": "output",
                    "digest": "<digest>", "item_count": 1, "byte_count": output_byte_count,
                    "source_revision": "<definition-digest>",
                    "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                    "source_metadata": {"definition_digest": "<definition-digest>"}},
                "evaluation_id": Uuid::nil(), "exit_code": 0, "stderr": "",
            }),
            json!({"payload_id": Uuid::nil(), "pointer": "/value", "data": [{"score": 12.5}],
                "cursor": 0, "next_cursor": 1, "total_count": 3, "digest": "<digest>"}),
            json!({"payload_id": Uuid::nil(), "pointer": "/value", "data": [{"score": 12.5}],
                "cursor": 0, "next_cursor": 1, "total_count": 3, "digest": "<digest>"}),
            json!({"payload_id": Uuid::nil(), "pointer": "/value", "data": [{"score": 20}],
                "cursor": 1, "next_cursor": 2, "total_count": 3, "digest": "<digest>"}),
            json!({"payload_id": Uuid::nil(), "pointer": "/args", "data": {"sample": "value"},
                "cursor": 0, "next_cursor": null, "total_count": 1, "digest": "<digest>"}),
            custom_indicator_evaluation::Model {
                id: Uuid::nil(),
                indicator_id,
                indicator_name: "sample-operation".into(),
                code: "<pinned-code>".into(),
                args: json!({"payload_id": Uuid::nil(), "pointers": {"result": "/value/1/score"},
                    "args": {"sample": "value"}}),
                output: json!({"result": 20}),
                evaluated_at: tests_common::ts_sentinel(),
                payload_id: Some(Uuid::nil()),
                payload_pointers: Some(json!({"result": "/value/1/score"})),
                execution_step_id: Some(execution_step_id),
            },
            1,
            vec![
                ExecRequest {
                    code: "<pinned-code>".into(),
                    stdin: Some(r#"{"args":{"sample":"value"}}"#.into()),
                    timeout: None,
                    max_output_bytes: Some(1024 * 1024),
                },
                ExecRequest {
                    code: "<pinned-code>".into(),
                    stdin: Some(r#"{"args":{"sample":"value"}}"#.into()),
                    timeout: None,
                    max_output_bytes: Some(1024 * 1024),
                },
            ],
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
            Err("-32602: task execution is outside the requested strategy scope".to_string()),
            Vec::<ExecRequest>::new(),
        ),
    );
}

fn normalize_operation(mut operation: Value) -> Value {
    for kind in ["input_payload", "output_payload"] {
        operation[kind]["id"] = json!(Uuid::nil());
        operation[kind]["task_id"] = json!(Uuid::nil());
        operation[kind]["digest"] = json!("<digest>");
        operation[kind]["source_revision"] = json!("<definition-digest>");
        operation[kind]["source_metadata"]["definition_digest"] = json!("<definition-digest>");
    }
    operation["evaluation_id"] = json!(Uuid::nil());
    operation
}

fn normalize_mcp_page(mut page: Value) -> Value {
    page["payload_id"] = json!(Uuid::nil());
    page["digest"] = json!("<digest>");
    page
}

fn normalize_evaluation(
    mut evaluation: custom_indicator_evaluation::Model,
) -> custom_indicator_evaluation::Model {
    evaluation.id = Uuid::nil();
    evaluation.code = "<pinned-code>".into();
    evaluation.args["payload_id"] = json!(Uuid::nil());
    evaluation.payload_id = Some(Uuid::nil());
    evaluation.evaluated_at = tests_common::ts_sentinel();
    evaluation
}
