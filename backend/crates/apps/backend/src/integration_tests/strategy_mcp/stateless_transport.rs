use std::{future::pending, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum_test::TestServer;
use chrono::Utc;
use core_application::agent_task_client::DisabledAgentTaskClient;
use core_application::kata_exec::{
    ExecRequest, ExecResult, FakeKataExecutor, KataExecError, KataExecutor, SharedKataExecutor,
};
use gateway_postgres::entities::{
    custom_indicator, strategy_task, task_execution_definition_snapshot,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, EntityTrait, QueryFilter,
};
use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use super::tests_common;
use crate::testing::insert_test_strategy;
use crate::testing::mcp::{legacy_initialize_body, parse_sse_response};

struct NotifyOnDrop(Arc<Notify>);

impl Drop for NotifyOnDrop {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

struct PendingKataExecutor {
    started: Arc<Notify>,
    dropped: Arc<Notify>,
}

#[async_trait]
impl KataExecutor for PendingKataExecutor {
    async fn run(&self, _request: ExecRequest) -> Result<ExecResult, KataExecError> {
        let _drop_notification = NotifyOnDrop(Arc::clone(&self.dropped));
        self.started.notify_one();
        pending().await
    }
}

async fn call_strategy_tool(
    server: &TestServer,
    request_id: u64,
    strategy_id: Uuid,
    strategy_task_id: Uuid,
    execution_id: &str,
    name: &str,
    arguments: Value,
) -> Value {
    let response = server
        .post("/mcp/strategy")
        .add_header("accept", "application/json, text/event-stream")
        .add_header("mcp-protocol-version", "2025-06-18")
        .add_header("x-strategy-id", strategy_id.to_string())
        .add_header("x-strategy-task-id", strategy_task_id.to_string())
        .add_header("x-execution-id", execution_id)
        .add_header("x-tool-models", "{}")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        }))
        .await;
    response.assert_status_ok();
    parse_sse_response(&response.text())
}

fn normalize_payload_ids_and_digests(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for item in values {
                normalize_payload_ids_and_digests(item);
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                if matches!(key.as_str(), "id" | "task_id" | "payload_id" | "purpose") {
                    if item.is_string() {
                        *item = json!(format!("<{key}>"));
                    }
                } else if matches!(key.as_str(), "digest" | "definition_digest") && item.is_string()
                {
                    *item = json!(format!("<{key}>"));
                } else if key == "source_revision" && item.is_string() {
                    *item = json!("<source_revision>");
                } else {
                    normalize_payload_ids_and_digests(item);
                }
            }
        }
        _ => {}
    }
}

#[backend_test_macros::database_test]
async fn strategy_tool_call_uses_request_headers_without_a_session(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let server = TestServer::new(crate::mcp::router(
        crate::services::use_cases::build_use_cases_without_agent_task_client(db),
        Arc::new(DisabledAgentTaskClient),
        None,
        None,
        None,
        None,
        Vec::new(),
    ))
    .expect("failed to build MCP test server");

    let initialize = server
        .post("/mcp/strategy")
        .add_header("accept", "application/json, text/event-stream")
        .json(&legacy_initialize_body())
        .await;

    let call = server
        .post("/mcp/strategy")
        .add_header("accept", "application/json, text/event-stream")
        .add_header("mcp-protocol-version", "2025-06-18")
        .add_header("x-strategy-id", strategy_id.to_string())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "list_notes", "arguments": {} },
        }))
        .await;

    assert_eq!(
        (
            initialize.status_code(),
            call.status_code(),
            call.headers().get("mcp-session-id").is_some(),
            parse_sse_response(&call.text()),
        ),
        (
            axum::http::StatusCode::OK,
            axum::http::StatusCode::OK,
            false,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "content": [{ "type": "text", "text": "{\"notes\":[]}" }],
                    "structuredContent": { "notes": [] },
                    "isError": false,
                },
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn task_only_execution_id_reads_snapshot_and_payload_over_streamable_http(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let task = tests_common::create_test_execution_task(&db, strategy_id, "phases: []").await;
    let task_id = task.task_id;
    let a2a_task_id = task.a2a_task_id;
    let task_start_snapshot = task_execution_definition_snapshot::Entity::find()
        .filter(task_execution_definition_snapshot::Column::TaskId.eq(task_id))
        .filter(task_execution_definition_snapshot::Column::DefinitionKey.eq("task_start"))
        .one(&db)
        .await
        .expect("query synthetic task start snapshot")
        .expect("task start snapshot exists");
    let snapshot_definition = task_start_snapshot.definition;

    let indicator_id = Uuid::new_v4();
    custom_indicator::ActiveModel {
        indicator_id: Set(indicator_id),
        name: Set("sample-operation".into()),
        scope: Set("strategy".into()),
        strategy_id: Set(Some(strategy_id)),
        code: Set("synthetic source".into()),
        input_schema: Set(json!({"type": "object"})),
        output_schema: Set(json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {"score": {"type": "integer"}},
                "required": ["score"],
                "additionalProperties": false,
            },
        })),
        description: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(&db)
    .await
    .expect("insert synthetic saved operation");

    let output_items: Vec<Value> = (0..101).map(|score| json!({"score": score})).collect();
    let executor = Arc::new(FakeKataExecutor::new());
    executor
        .set_response(Ok(ExecResult {
            stdout: format!(
                "{}\n",
                serde_json::to_string(&output_items).expect("serialize output")
            ),
            stderr: String::new(),
            exit_code: 0,
            files: Vec::new(),
        }))
        .await;
    let shared_executor: SharedKataExecutor = executor;
    let server = TestServer::new(crate::mcp::router(
        crate::services::use_cases::build_use_cases_without_agent_task_client(db.clone()),
        Arc::new(DisabledAgentTaskClient),
        None,
        Some(shared_executor),
        None,
        None,
        Vec::new(),
    ))
    .expect("build MCP test server");

    let initialize = server
        .post("/mcp/strategy")
        .add_header("accept", "application/json, text/event-stream")
        .json(&legacy_initialize_body())
        .await;
    initialize.assert_status_ok();

    let snapshot = call_strategy_tool(
        &server,
        2,
        strategy_id,
        task_id,
        &a2a_task_id,
        "read_task_execution_snapshot",
        json!({}),
    )
    .await;
    let execution_step_id = Uuid::new_v4();
    let operation = call_strategy_tool(
        &server,
        3,
        strategy_id,
        task_id,
        &format!("{a2a_task_id}:{execution_step_id}"),
        "execute_operation",
        json!({
            "name": "sample-operation",
            "args": {"sample": "value"},
            "timeout_secs": null,
            "max_output_bytes": null,
        }),
    )
    .await;
    let output_payload_id = operation["result"]["structuredContent"]["output_payload"]["id"]
        .as_str()
        .expect("operation output payload ID");

    let partial_read = call_strategy_tool(
        &server,
        4,
        strategy_id,
        task_id,
        &a2a_task_id,
        "read_execution_payload",
        json!({
            "payload_id": output_payload_id,
            "pointer": "/value",
            "cursor": 4,
            "limit": 2,
        }),
    )
    .await;
    let first_for_each_page = call_strategy_tool(
        &server,
        5,
        strategy_id,
        task_id,
        &a2a_task_id,
        "read_execution_payload",
        json!({
            "payload_id": output_payload_id,
            "pointer": "/value",
            "cursor": 0,
            "limit": 100,
        }),
    )
    .await;
    let second_for_each_page = call_strategy_tool(
        &server,
        6,
        strategy_id,
        task_id,
        &a2a_task_id,
        "read_execution_payload",
        json!({
            "payload_id": output_payload_id,
            "pointer": "/value",
            "cursor": 100,
            "limit": 100,
        }),
    )
    .await;

    let other_strategy_id = insert_test_strategy(&db, "other-sample-strategy").await;
    let completed_task_id = crate::testing::insert_test_strategy_task(
        &db,
        strategy_id,
        "synthetic completed task",
        None,
        Utc::now().fixed_offset(),
    )
    .await;
    let completed_a2a_task_id = format!("sample-completed-task-{}", Uuid::new_v4());
    strategy_task::ActiveModel {
        task_id: Set(completed_task_id),
        a2a_task_id: Set(Some(completed_a2a_task_id.clone())),
        ..Default::default()
    }
    .update(&db)
    .await
    .expect("attach A2A ID to completed task");
    let unknown_task = call_strategy_tool(
        &server,
        7,
        strategy_id,
        task_id,
        "sample-unknown-task",
        "read_task_execution_snapshot",
        json!({}),
    )
    .await;
    let wrong_strategy = call_strategy_tool(
        &server,
        8,
        other_strategy_id,
        task_id,
        &a2a_task_id,
        "read_task_execution_snapshot",
        json!({}),
    )
    .await;
    let completed_task = call_strategy_tool(
        &server,
        9,
        strategy_id,
        completed_task_id,
        &completed_a2a_task_id,
        "read_task_execution_snapshot",
        json!({}),
    )
    .await;

    let snapshot_output = snapshot["result"]["structuredContent"].clone();
    let operation_output = operation["result"]["structuredContent"].clone();
    let partial_output = partial_read["result"]["structuredContent"].clone();
    let first_page_output = first_for_each_page["result"]["structuredContent"].clone();
    let second_page_output = second_for_each_page["result"]["structuredContent"].clone();
    let auth_errors = json!({
        "unknown_task": unknown_task["error"].clone(),
        "wrong_strategy": wrong_strategy["error"].clone(),
        "completed_task": completed_task["error"].clone(),
    });
    let mut actual = json!({
        "snapshot": snapshot_output,
        "operation": operation_output,
        "partial_read": partial_output,
        "first_for_each_page": first_page_output,
        "second_for_each_page": second_page_output,
        "auth_errors": auth_errors,
    });
    normalize_payload_ids_and_digests(&mut actual);

    let mut expected = json!({
        "snapshot": {
            "definition": snapshot_definition,
            "digest": "<digest>",
        },
        "operation": {
            "input_payload": {
                "id": "<id>",
                "task_id": "<task_id>",
                "kind": "input",
                "digest": "<digest>",
                "item_count": 1,
                "byte_count": 27,
                "source_revision": "<source_revision>",
                "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                "source_metadata": {"definition_digest": "<definition_digest>"},
            },
            "output_payload": {
                "id": "<id>",
                "task_id": "<task_id>",
                "kind": "output",
                "digest": "<digest>",
                "item_count": 1,
                "byte_count": 2862,
                "source_revision": "<source_revision>",
                "source_locator": {"definition_key": "custom_indicator:sample-operation"},
                "source_metadata": {"definition_digest": "<definition_digest>"},
            },
            "evaluation_id": null,
            "projection_error": null,
            "exit_code": 0,
            "stderr": "",
        },
        "partial_read": {
            "payload_id": "<payload_id>",
            "pointer": "/value",
            "data": output_items[4..6].to_vec(),
            "cursor": 4,
            "next_cursor": 6,
            "total_count": 101,
            "digest": "<digest>",
        },
        "first_for_each_page": {
            "payload_id": "<payload_id>",
            "pointer": "/value",
            "data": output_items[..100].to_vec(),
            "cursor": 0,
            "next_cursor": 100,
            "total_count": 101,
            "digest": "<digest>",
        },
        "second_for_each_page": {
            "payload_id": "<payload_id>",
            "pointer": "/value",
            "data": output_items[100..].to_vec(),
            "cursor": 100,
            "next_cursor": null,
            "total_count": 101,
            "digest": "<digest>",
        },
        "auth_errors": {
            "unknown_task": {"code": -32602, "message": "execution does not match an active task"},
            "wrong_strategy": {"code": -32602, "message": "execution does not match an active task"},
            "completed_task": {"code": -32602, "message": "execution does not match an active task"},
        },
    });
    normalize_payload_ids_and_digests(&mut expected);

    assert_eq!(actual, expected);
}

#[backend_test_macros::database_test]
async fn cancelling_an_in_flight_operation_request_stops_the_kata_handler(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let task = tests_common::create_test_execution_task(&db, strategy_id, "phases: []").await;
    let task_id = task.task_id;
    let a2a_task_id = task.a2a_task_id;

    custom_indicator::ActiveModel {
        indicator_id: Set(Uuid::new_v4()),
        name: Set("sample-operation".into()),
        scope: Set("strategy".into()),
        strategy_id: Set(Some(strategy_id)),
        code: Set("synthetic source".into()),
        input_schema: Set(json!({"type": "object"})),
        output_schema: Set(json!({"type": "object"})),
        description: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(&db)
    .await
    .expect("insert synthetic saved operation");

    let started = Arc::new(Notify::new());
    let dropped = Arc::new(Notify::new());
    let shared_executor: SharedKataExecutor = Arc::new(PendingKataExecutor {
        started: Arc::clone(&started),
        dropped: Arc::clone(&dropped),
    });
    let server = TestServer::new(crate::mcp::router(
        crate::services::use_cases::build_use_cases_without_agent_task_client(db),
        Arc::new(DisabledAgentTaskClient),
        None,
        Some(shared_executor),
        None,
        None,
        Vec::new(),
    ))
    .expect("build MCP test server");
    let execution_id = format!("{a2a_task_id}:{}", Uuid::new_v4());
    let in_flight_request = tokio::spawn(async move {
        call_strategy_tool(
            &server,
            20,
            strategy_id,
            task_id,
            &execution_id,
            "execute_operation",
            json!({
                "name": "sample-operation",
                "args": {},
                "timeout_secs": null,
                "max_output_bytes": null,
            }),
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(5), started.notified())
        .await
        .expect("Kata execution starts");
    in_flight_request.abort();
    let request_result = in_flight_request.await;
    let execution_drop = tokio::time::timeout(Duration::from_secs(5), dropped.notified()).await;

    assert_eq!(
        (
            request_result.is_err_and(|error| error.is_cancelled()),
            execution_drop.is_ok(),
        ),
        (true, true),
    );
}
