use axum::http::{HeaderMap, HeaderValue};
use indoc::indoc;
use rmcp::ErrorData as McpError;
use serde_json::{Value, json};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::tests_common::mock_db_with_strategy;
use crate::integration_tests::mcp_tool::call_tool_output_with_headers;
use crate::mcp::StrategyServer;
use crate::services::litellm_client::LiteLlmClient;
use crate::services::use_cases::build_use_cases;

fn server(
    db: impl Into<gateway_postgres::DatabaseHandle>,
    llm_client: Option<core_application::llm_client::SharedLlmClient>,
) -> StrategyServer {
    let use_cases = build_use_cases(db);
    StrategyServer::new(crate::mcp::strategy_server_dependencies(
        &use_cases, None, None, llm_client,
    ))
}

fn headers(strategy_id: Uuid) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy id"),
    );
    headers.insert(
        "x-tool-models",
        HeaderValue::from_static(
            r#"{"search_web":"example-model-search","query_media":"example-model-media"}"#,
        ),
    );
    headers
}

#[tokio::test]
async fn query_media_uses_model_from_tool_models_header() {
    let strategy_id = Uuid::new_v4();
    let litellm = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{"message": {"content": "mock media summary"}}],
        })))
        .mount(&litellm)
        .await;

    let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
    let server = server(
        mock_db_with_strategy(strategy_id),
        Some(std::sync::Arc::new(client)),
    );
    let result = call_tool_output_with_headers::<_, Value>(
        &server,
        "query_media",
        json!({
            "media_url": "https://example.com/video.mp4",
            "prompt": "summarize the clip",
        }),
        headers(strategy_id),
    )
    .await;
    let requests = litellm
        .received_requests()
        .await
        .expect("recorded requests");
    let body: Value = requests[0].body_json().expect("parse request body");

    assert_eq!(
        (result, body),
        (
            Ok(json!({"text": "mock media summary"})),
            json!({
                "model": "example-model-media",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "summarize the clip"},
                        {"type": "file", "file": {"file_id": "https://example.com/video.mp4"}},
                    ],
                }],
            }),
        ),
    );
}

#[tokio::test]
async fn search_web_uses_model_from_tool_models_header() {
    let strategy_id = Uuid::new_v4();
    let litellm = MockServer::start().await;
    let mut response_body = format!(
        indoc! {"
            data: {}

            data: [DONE]
        "},
        json!({"choices": [{"delta": {"content": "mock search result"}}]})
    );
    response_body.push('\n');
    response_body.push('\n');
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
        .mount(&litellm)
        .await;

    let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
    let server = server(
        mock_db_with_strategy(strategy_id),
        Some(std::sync::Arc::new(client)),
    );
    let result = call_tool_output_with_headers::<_, Value>(
        &server,
        "search_web",
        json!({"query": "example query"}),
        headers(strategy_id),
    )
    .await;
    let requests = litellm
        .received_requests()
        .await
        .expect("recorded requests");
    let body: Value = requests[0].body_json().expect("parse request body");

    assert_eq!(
        (result, body),
        (
            Ok(json!({"text": "mock search result", "citations": []})),
            json!({
                "model": "example-model-search",
                "messages": [{
                    "role": "user",
                    "content": [{"type": "text", "text": "example query"}],
                }],
                "stream": true,
                "web_search_options": {},
                "allowed_openai_params": ["web_search_options"],
            }),
        ),
    );
}

#[tokio::test]
async fn query_data_rejects_nonexistent_strategy() {
    let strategy_id = Uuid::new_v4();
    let db = sea_orm::MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
        .append_query_results([Vec::<gateway_postgres::entities::strategy::Model>::new()])
        .into_connection();
    let server = server(db, None);

    assert_eq!(
        call_tool_output_with_headers::<_, Value>(
            &server,
            "query_data",
            json!({
                "instrument_ids": [],
                "from": "2025-01-01",
                "to": "2025-01-01",
            }),
            headers_without_models(strategy_id),
        )
        .await,
        Err(McpError::invalid_params(
            format!("strategy {strategy_id} not found"),
            None,
        )),
    );
}

fn headers_without_models(strategy_id: Uuid) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy id"),
    );
    headers
}
