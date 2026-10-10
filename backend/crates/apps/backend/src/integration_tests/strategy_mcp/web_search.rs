use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use core_application::web_search::{
    SharedWebSearchClient, WebSearchClient, WebSearchError, WebSearchResult, WebSearchTimeRange,
    WebSearchTopic,
};
use serde_json::json;
use uuid::Uuid;

use super::SEARCH_WEB_MAX_CALLS_PER_TASK;
use super::dto::SearchWebParams;
use super::tests_common::{build_server, insert_strategy};

#[derive(Clone)]
struct FakeWebSearchClient {
    calls: Arc<AtomicUsize>,
    results: Vec<WebSearchResult>,
    fails: bool,
}

#[async_trait]
impl WebSearchClient for FakeWebSearchClient {
    async fn search(
        &self,
        _query: &str,
        _topic: Option<WebSearchTopic>,
        _time_range: Option<WebSearchTimeRange>,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.fails {
            return Err(WebSearchError::Api {
                status: 500,
                message: "upstream error".into(),
            });
        }
        Ok(self.results.clone())
    }
}

fn client(results: Vec<WebSearchResult>, fails: bool) -> (SharedWebSearchClient, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
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
        body: Some("# Example article body".into()),
        body_truncated: false,
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
        (result.as_json().clone(), calls.load(Ordering::Relaxed)),
        (
            json!({
                "results": [{
                    "title": "Example article",
                    "url": "https://example.invalid/article",
                    "published_date": "2026-04-05",
                    "snippet": "Example snippet",
                    "body": "# Example article body",
                    "body_truncated": false,
                }],
            }),
            1,
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
        (err, calls.load(Ordering::Relaxed)),
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
        calls.load(Ordering::Relaxed),
        (SEARCH_WEB_MAX_CALLS_PER_TASK * 2) as usize,
    );
}
