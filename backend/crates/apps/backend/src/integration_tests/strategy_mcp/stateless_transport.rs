use std::sync::Arc;

use axum_test::TestServer;
use core_application::agent_task_client::DisabledAgentTaskClient;
use serde_json::{Value, json};

use crate::testing::insert_test_strategy;

fn parse_sse_response(body: &str) -> Value {
    body.lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .find_map(|payload| serde_json::from_str(payload).ok())
        .expect("no JSON data line in MCP response")
}

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
        Vec::new(),
    ))
    .expect("failed to build MCP test server");

    let initialize = server
        .post("/mcp/strategy")
        .add_header("accept", "application/json, text/event-stream")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test-client", "version": "0.0.0" },
            },
        }))
        .await;
    let initialize_session_id = initialize.headers().get("mcp-session-id").is_some();

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
            initialize_session_id,
            call.status_code(),
            call.headers().get("mcp-session-id").is_some(),
            parse_sse_response(&call.text()),
        ),
        (
            axum::http::StatusCode::OK,
            false,
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
