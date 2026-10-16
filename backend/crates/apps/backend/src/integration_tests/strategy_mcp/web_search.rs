use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use core_application::web_search::{
    SharedWebSearchClient, WebSearchClient, WebSearchError, WebSearchResult, WebSearchTimeRange,
    WebSearchTopic,
};
use serde_json::json;
use uuid::Uuid;

use super::super::mcp_tool::call_tool_output_with_headers;
use super::SEARCH_WEB_MAX_CALLS_PER_TASK;
use super::dto::SearchWebParams;
use super::tests_common::{
    build_server, create_task_with_subagent_limits, insert_strategy, insert_subagent_headers,
};

#[derive(Clone)]
struct FakeWebSearchClient {
    calls: Arc<Mutex<Vec<SearchCall>>>,
    results: Vec<WebSearchResult>,
    fails: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchCall {
    query: String,
    topic: Option<WebSearchTopic>,
    time_range: Option<WebSearchTimeRange>,
}

#[async_trait]
impl WebSearchClient for FakeWebSearchClient {
    async fn search(
        &self,
        query: &str,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        self.calls.lock().expect("record calls").push(SearchCall {
            query: query.to_owned(),
            topic,
            time_range,
        });
        if self.fails {
            return Err(WebSearchError::Api {
                status: 500,
                message: "upstream error".into(),
            });
        }
        Ok(self.results.clone())
    }

    async fn extract_page(&self, _url: &str) -> Result<String, WebSearchError> {
        if self.fails {
            return Err(WebSearchError::ExtractionFailed {
                url: "https://example.invalid/article".into(),
                message: "page is unavailable".into(),
            });
        }
        Ok("# Example page".into())
    }
}

fn client(
    results: Vec<WebSearchResult>,
    fails: bool,
) -> (SharedWebSearchClient, Arc<Mutex<Vec<SearchCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(FakeWebSearchClient {
            calls: calls.clone(),
            results,
            fails,
        }),
        calls,
    )
}

fn params(query: &str) -> SearchWebParams {
    SearchWebParams {
        query: query.into(),
        topic: None,
        time_range: None,
    }
}

fn example_result() -> WebSearchResult {
    WebSearchResult {
        title: "Example article".into(),
        url: "https://example.invalid/article".into(),
        published_date: Some("2026-04-05".into()),
        snippet: "Example snippet".into(),
    }
}

#[backend_test_macros::database_test]
async fn search_web_requires_tavily_api_key(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let server = build_server(db);
    let err = server
        .search_web(strategy_id, None, params("example query"))
        .await
        .expect_err("expected internal error");
    assert_eq!(
        err,
        rmcp::ErrorData::internal_error("TAVILY_API_KEY is not configured", None),
    );
}

#[backend_test_macros::database_test]
async fn search_web_rejects_empty_query(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let server = build_server(db);
    let err = server
        .search_web(strategy_id, None, params("   "))
        .await
        .expect_err("expected invalid params");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params("query must not be empty", None),
    );
}

#[backend_test_macros::database_test]
async fn search_web_returns_article_results_without_tool_model_header(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));
    let mut params = params("example query");
    params.topic = Some("news".into());
    params.time_range = Some("month".into());

    let result = server
        .search_web(strategy_id, None, params)
        .await
        .expect("search web");

    assert_eq!(
        (
            result.as_json().clone(),
            calls.lock().expect("record calls").clone()
        ),
        (
            json!({
                "results": [{
                    "title": "Example article",
                    "url": "https://example.invalid/article",
                    "published_date": "2026-04-05",
                    "snippet": "Example snippet",
                }],
            }),
            vec![SearchCall {
                query: "example query".into(),
                topic: Some(WebSearchTopic::News),
                time_range: Some(WebSearchTimeRange::Month),
            }],
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_enforces_per_task_call_limit(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));
    let task_execution_id = format!("task-{}", Uuid::new_v4());

    for _ in 0..SEARCH_WEB_MAX_CALLS_PER_TASK {
        server
            .search_web(
                strategy_id,
                Some(task_execution_id.clone()),
                params("example query"),
            )
            .await
            .expect("call within limit should succeed");
    }

    let err = server
        .search_web(
            strategy_id,
            Some(task_execution_id.clone()),
            params("example query"),
        )
        .await
        .expect_err("call beyond limit should fail");
    assert_eq!(
        (err, calls.lock().expect("record calls").len()),
        (
            rmcp::ErrorData::invalid_params(
                format!(
                    "search_web call limit ({SEARCH_WEB_MAX_CALLS_PER_TASK}) exceeded for this task execution"
                ),
                None,
            ),
            SEARCH_WEB_MAX_CALLS_PER_TASK as usize,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_releases_call_count_reservation_when_request_fails(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let (client, calls) = client(vec![], true);
    let server = build_server(db).with_web_search_client(Some(client));
    let task_execution_id = format!("task-{}", Uuid::new_v4());

    for _ in 0..(SEARCH_WEB_MAX_CALLS_PER_TASK * 2) {
        let err = server
            .search_web(
                strategy_id,
                Some(task_execution_id.clone()),
                params("example query"),
            )
            .await
            .expect_err("upstream failure should propagate");
        assert_eq!(
            err,
            rmcp::ErrorData::internal_error(
                "web search failed: web search API error (status 500): upstream error",
                None,
            ),
        );
    }
    assert_eq!(
        calls.lock().expect("record calls").len(),
        (SEARCH_WEB_MAX_CALLS_PER_TASK * 2) as usize,
    );
}

#[backend_test_macros::database_test]
async fn search_web_releases_subagent_call_count_reservation_when_request_fails(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution_id =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 4).await;
    let (client, calls) = client(vec![], true);
    let server = build_server(db).with_web_search_client(Some(client));

    let first_call = invoke_search(
        &server,
        strategy_id,
        &task_execution_id,
        Some(("child-run-a", "researcher")),
    )
    .await;
    let retry = invoke_search(
        &server,
        strategy_id,
        &task_execution_id,
        Some(("child-run-a", "researcher")),
    )
    .await;

    assert_eq!(
        (first_call, retry, calls.lock().expect("record calls").len()),
        (
            Err(rmcp::ErrorData::internal_error(
                "web search failed: web search API error (status 500): upstream error",
                None,
            )),
            Err(rmcp::ErrorData::internal_error(
                "web search failed: web search API error (status 500): upstream error",
                None,
            )),
            2,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_enforces_subagent_and_task_limits(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution_id =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 4).await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));

    let phase_call = invoke_search(&server, strategy_id, &task_execution_id, None).await;
    let first_subagent_call = invoke_search(
        &server,
        strategy_id,
        &task_execution_id,
        Some(("child-run-a", "researcher")),
    )
    .await;
    let repeated_subagent_call = invoke_search(
        &server,
        strategy_id,
        &task_execution_id,
        Some(("child-run-a", "researcher")),
    )
    .await;
    let other_subagent_call = invoke_search(
        &server,
        strategy_id,
        &task_execution_id,
        Some(("child-run-b", "researcher")),
    )
    .await;
    let second_phase_call = invoke_search(&server, strategy_id, &task_execution_id, None).await;
    let call_beyond_task_limit =
        invoke_search(&server, strategy_id, &task_execution_id, None).await;

    assert_eq!(
        (
            phase_call,
            first_subagent_call,
            repeated_subagent_call,
            other_subagent_call,
            second_phase_call,
            call_beyond_task_limit,
            calls.lock().expect("record calls").len(),
        ),
        (
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }]})),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }]})),
            Err(rmcp::ErrorData::invalid_params(
                "search_web call limit (1) exceeded for this subagent execution",
                None,
            )),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }]})),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }]})),
            Err(rmcp::ErrorData::invalid_params(
                "search_web call limit (4) exceeded for this task execution",
                None,
            )),
            4,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_rejects_unknown_subagent_definition_key(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution_id =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 4).await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));

    let result = invoke_search(
        &server,
        strategy_id,
        &task_execution_id,
        Some(("child-run-a", "unknown-researcher")),
    )
    .await;

    assert_eq!(
        (result, calls.lock().expect("record calls").len()),
        (
            Err(rmcp::ErrorData::invalid_params(
                "subagent definition 'unknown-researcher' not found",
                None,
            )),
            0,
        ),
    );
}

async fn invoke_search(
    server: &super::test_server::StrategyServer,
    strategy_id: Uuid,
    task_execution_id: &str,
    subagent: Option<(&str, &str)>,
) -> Result<serde_json::Value, rmcp::ErrorData> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy ID"),
    );
    headers.insert(
        "x-execution-id",
        HeaderValue::from_str(&format!("{task_execution_id}:{}", Uuid::new_v4()))
            .expect("valid execution ID"),
    );
    insert_subagent_headers(&mut headers, subagent);
    call_tool_output_with_headers::<_, serde_json::Value>(
        server.entrypoint_server(),
        "search_web",
        serde_json::to_value(params("example query")).expect("serialize params"),
        headers,
    )
    .await
}
