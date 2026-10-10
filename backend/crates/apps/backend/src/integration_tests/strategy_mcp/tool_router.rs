use axum::http::{HeaderMap, HeaderValue};
use rmcp::ErrorData as McpError;
use rstest::rstest;
use serde_json::{Value, json};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::tests_common::mock_db_with_strategy;
use crate::integration_tests::mcp_tool::call_tool_output_with_headers;
use crate::services::use_cases::build_use_cases;
use entrypoint_agent_mcp::StrategyServer;
use gateway_litellm::LiteLlmClient;

fn server(
    db: impl Into<gateway_postgres::DatabaseHandle>,
    llm_client: Option<core_application::llm_client::SharedLlmClient>,
) -> StrategyServer {
    let use_cases = build_use_cases(db);
    StrategyServer::new(crate::mcp::strategy_server_dependencies(
        &use_cases, None, None, llm_client, None,
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
        HeaderValue::from_static(r#"{"query_youtube":"example-model-youtube"}"#),
    );
    headers
}

#[rstest]
#[case::youtube_com("https://youtube.com/watch?v=sample-video-id")]
#[case::www_youtube_com("https://www.youtube.com/watch?v=sample-video-id")]
#[case::m_youtube_com("https://m.youtube.com/watch?v=sample-video-id")]
#[case::youtu_be("https://youtu.be/sample-video-id")]
#[tokio::test]
async fn query_youtube_uses_model_from_tool_models_header(#[case] youtube_url: &str) {
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
        "query_youtube",
        json!({
            "youtube_url": youtube_url,
            "questions": ["summarize the clip", "what numerical values are mentioned?"],
        }),
        headers(strategy_id),
    )
    .await;
    let requests = litellm
        .received_requests()
        .await
        .expect("recorded requests");
    let bodies = requests
        .into_iter()
        .map(|request| request.body_json().expect("parse request body"))
        .collect::<Vec<Value>>();

    assert_eq!(
        (result, bodies),
        (
            Ok(json!({"text": "mock media summary"})),
            vec![json!({
                "model": "example-model-youtube",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "Answer each question about this video. Include an MM:SS timestamp whenever you mention a numerical value or attribute a statement to the video. Questions: 1. summarize the clip; 2. what numerical values are mentioned?"},
                        {"type": "file", "file": {"file_id": youtube_url, "format": "video/mp4"}},
                    ],
                }],
            })],
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
