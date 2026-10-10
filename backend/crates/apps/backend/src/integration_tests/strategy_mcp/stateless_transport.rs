use std::sync::Arc;

use axum_test::TestServer;
use core_application::agent_task_client::DisabledAgentTaskClient;
use serde_json::json;

use crate::testing::insert_test_strategy;
use crate::testing::mcp::{legacy_initialize_body, parse_sse_response};

#[backend_test_macros::database_test]
async fn strategy_tool_call_uses_request_headers_without_a_session(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_test_strategy(&db, "sample-strategy").await;
    let server = TestServer::new(crate::mcp::router(
        crate::services::use_cases::build_use_cases(db),
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
