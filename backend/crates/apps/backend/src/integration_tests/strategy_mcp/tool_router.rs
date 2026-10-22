use axum::http::{HeaderMap, HeaderValue};
use core_application::agent_task_client::FakeAgentTaskClient;
use core_application::strategy_task::TaskSource;
use indoc::indoc;
use rmcp::ErrorData as McpError;
use rstest::rstest;
use serde_json::{Value, json};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::tests_common::mock_db_with_strategy;
use crate::integration_tests::mcp_tool::{
    call_tool_output_with_headers, call_tool_output_with_headers_and_id,
};
use crate::services::use_cases::build_use_cases_without_agent_task_client;
use entrypoint_agent_mcp::StrategyServer;
use gateway_litellm::LiteLlmClient;

fn server(
    db: impl Into<gateway_postgres::DatabaseHandle>,
    llm_client: Option<core_application::llm_client::SharedLlmClient>,
) -> StrategyServer {
    let use_cases = build_use_cases_without_agent_task_client(db);
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

#[tokio::test]
async fn rejects_agent_requests_without_the_stable_task_id_header() {
    let strategy_id = Uuid::new_v4();
    let mut headers = headers_without_models(strategy_id);
    headers.insert(
        "x-execution-id",
        HeaderValue::from_static("synthetic-attempt:00000000-0000-0000-0000-000000000001"),
    );
    let server = server(mock_db_with_strategy(strategy_id), None);

    assert_eq!(
        call_tool_output_with_headers::<_, Value>(
            &server,
            "query_data",
            json!({
                "instrument_ids": [],
                "from": "2025-01-01",
                "to": "2025-01-01",
            }),
            headers,
        )
        .await,
        Err(McpError::invalid_params(
            "missing x-strategy-task-id header",
            None,
        )),
    );
}

#[backend_test_macros::database_test]
async fn replays_a_completed_mcp_request_and_rejects_changed_arguments(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = super::tests_common::insert_strategy(&db, "synthetic strategy").await;
    let purpose = format!("synthetic-purpose-{}", Uuid::new_v4());
    let use_cases = build_use_cases_without_agent_task_client(db.clone());
    use_cases
        .agent_configs()
        .create(purpose.clone())
        .await
        .expect("create synthetic agent config");
    use_cases
        .agent_configs()
        .save_agent_graph(
            &purpose,
            indoc! {"
                execution_limits:
                  max_tool_calls: 1
                phases: []
            "},
        )
        .await
        .expect("save synthetic task allocation");
    let fake = FakeAgentTaskClient::new();
    fake.set_next_task_id("synthetic-attempt").await;
    let task = use_cases
        .strategy_tasks()
        .submit_task(
            &fake,
            strategy_id,
            "synthetic prompt",
            TaskSource::Review,
            Some(purpose),
        )
        .await
        .expect("submit synthetic task");

    let llm = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(1))
                .set_body_json(json!({
                    "choices": [{"message": {"content": "synthetic response"}}],
                })),
        )
        .mount(&llm)
        .await;
    let llm_client = LiteLlmClient::new(&llm.uri(), None).expect("build synthetic LLM client");
    let server = server(db, Some(std::sync::Arc::new(llm_client)));
    let mut request_headers = headers(strategy_id);
    request_headers.insert(
        "x-strategy-task-id",
        HeaderValue::from_str(&task.task_id.to_string()).expect("valid task UUID"),
    );
    request_headers.insert(
        "x-execution-id",
        HeaderValue::from_str(&format!("{}:{}", task.a2a_task_id, Uuid::from_u128(401),))
            .expect("valid attempt and step header"),
    );
    let first_arguments = json!({
        "youtube_url": "https://youtube.com/watch?v=synthetic-video",
        "questions": ["Describe the synthetic scene."],
    });
    let (first, concurrent_replay) = tokio::join!(
        call_tool_output_with_headers_and_id::<_, Value>(
            &server,
            "query_youtube",
            first_arguments.clone(),
            rmcp::model::NumberOrString::Number(1),
            request_headers.clone(),
        ),
        call_tool_output_with_headers_and_id::<_, Value>(
            &server,
            "query_youtube",
            first_arguments.clone(),
            rmcp::model::NumberOrString::Number(1),
            request_headers.clone(),
        ),
    );
    let mut concurrent_outcomes = [first, concurrent_replay]
        .map(|result| match result {
            Ok(response) if response == json!({"text": "synthetic response"}) => "completed",
            Err(error)
                if error.message == "tool call dispatch is already claimed"
                    || error.message
                        == "tool call is already dispatched and its result is unavailable" =>
            {
                "in_flight"
            }
            _ => "unexpected",
        })
        .into_iter()
        .collect::<Vec<_>>();
    concurrent_outcomes.sort_unstable();
    let replay = call_tool_output_with_headers_and_id::<_, Value>(
        &server,
        "query_youtube",
        first_arguments.clone(),
        rmcp::model::NumberOrString::Number(1),
        request_headers.clone(),
    )
    .await;
    let changed = call_tool_output_with_headers_and_id::<_, Value>(
        &server,
        "query_youtube",
        json!({
            "youtube_url": "https://youtube.com/watch?v=synthetic-video",
            "questions": ["Describe another synthetic scene."],
        }),
        rmcp::model::NumberOrString::Number(1),
        request_headers,
    )
    .await
    .map_err(|error| (error.code, error.message.to_string()));
    let request_count = llm
        .received_requests()
        .await
        .expect("record synthetic LLM requests")
        .len();
    assert_eq!(
        (
            matches!(
                concurrent_outcomes.as_slice(),
                ["completed", "in_flight"] | ["completed", "completed"]
            ),
            replay,
            changed,
            request_count,
        ),
        (
            true,
            Ok(json!({"text": "synthetic response"})),
            Err((
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "execution reservation identifier conflicts with an existing call".to_string(),
            )),
            1,
        ),
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
