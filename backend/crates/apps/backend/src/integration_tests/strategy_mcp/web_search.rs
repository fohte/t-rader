use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use core_application::web_search::{
    SharedWebSearchClient, WebPageExtractionResponse, WebSearchAccountUsage, WebSearchClient,
    WebSearchError, WebSearchResponse, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
};
use rust_decimal::Decimal;
use serde_json::json;
use uuid::Uuid;

use super::super::mcp_tool::{call_tool_output_with_headers, call_tool_output_with_headers_and_id};
use super::SEARCH_WEB_MAX_CALLS_PER_TASK;
use super::dto::SearchWebParams;
use super::test_credit_budget::expected_denial;
use super::tests_common::{
    TestExecutionContext, build_server, create_task_with_subagent_limits, insert_strategy,
    insert_subagent_headers,
};
use core_application::tavily_credit_budget::{TavilyCreditBudgetReason, TavilyCreditBudgetScope};

const RETRIEVED_AT: &str = "2026-01-01T00:00:00Z";

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
    ) -> Result<WebSearchResponse, WebSearchError> {
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
        Ok(WebSearchResponse {
            results: self.results.clone(),
            credits_used: Some(Decimal::ONE),
        })
    }

    async fn extract_page(&self, url: &str) -> Result<WebPageExtractionResponse, WebSearchError> {
        if self.fails {
            return Ok(WebPageExtractionResponse {
                raw_content: None,
                failure: Some(core_application::web_search::WebPageExtractionFailure {
                    url: url.to_owned(),
                    message: "page is unavailable".into(),
                }),
                credits_used: Some(Decimal::ZERO),
            });
        }
        Ok(WebPageExtractionResponse {
            raw_content: Some("# Example page".into()),
            failure: None,
            credits_used: Some(Decimal::ONE),
        })
    }

    async fn account_usage(&self) -> Result<WebSearchAccountUsage, WebSearchError> {
        Ok(WebSearchAccountUsage {
            current_plan: "Researcher".into(),
            plan_credits_used: Decimal::ZERO,
            paygo_credits_used: Decimal::ZERO,
        })
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
async fn search_web_stops_when_credit_budget_is_missing(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let server = build_server(db);
    let err = server
        .search_web(strategy_id, None, params("example query"))
        .await
        .expect_err("expected structured budget denial");
    assert_eq!(
        err,
        expected_denial(
            TavilyCreditBudgetReason::ConfigurationUnavailable,
            TavilyCreditBudgetScope::Account,
            None,
        ),
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
                "retrieved_at": "2026-01-01T00:00:00Z",
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
async fn direct_search_web_calls_with_reused_json_rpc_id_get_distinct_reservations(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));
    let mut request_headers = HeaderMap::new();
    request_headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy ID"),
    );
    let arguments = serde_json::to_value(params("example query")).expect("serialize params");

    let first = call_tool_output_with_headers_and_id::<_, serde_json::Value>(
        server.entrypoint_server(),
        "search_web",
        arguments.clone(),
        rmcp::model::NumberOrString::Number(1),
        request_headers.clone(),
    )
    .await;
    let second = call_tool_output_with_headers_and_id::<_, serde_json::Value>(
        server.entrypoint_server(),
        "search_web",
        arguments,
        rmcp::model::NumberOrString::Number(1),
        request_headers,
    )
    .await;

    assert_eq!(
        (first, second, calls.lock().expect("record calls").len()),
        (
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            2,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_replays_the_same_task_request_without_a_second_provider_call(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 20).await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));
    let mut request_headers = HeaderMap::new();
    request_headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy ID"),
    );
    request_headers.insert(
        "x-strategy-task-id",
        HeaderValue::from_str(&task_execution.task_id.to_string()).expect("valid task ID"),
    );
    request_headers.insert(
        "x-execution-id",
        HeaderValue::from_str(&format!(
            "{}:{}",
            task_execution.a2a_task_id,
            Uuid::from_u128(305),
        ))
        .expect("valid execution ID"),
    );
    let arguments = serde_json::to_value(params("example query")).expect("serialize params");

    let first = call_tool_output_with_headers_and_id::<_, serde_json::Value>(
        server.entrypoint_server(),
        "search_web",
        arguments.clone(),
        rmcp::model::NumberOrString::Number(1),
        request_headers.clone(),
    )
    .await;
    let replay = call_tool_output_with_headers_and_id::<_, serde_json::Value>(
        server.entrypoint_server(),
        "search_web",
        arguments,
        rmcp::model::NumberOrString::Number(1),
        request_headers,
    )
    .await;

    assert_eq!(
        (first, replay, calls.lock().expect("record calls").len()),
        (
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            1,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_enforces_per_task_call_limit(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let (client, calls) = client(vec![example_result()], false);
    let task_execution =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 20).await;
    let server = build_server(db).with_web_search_client(Some(client));

    for _ in 0..SEARCH_WEB_MAX_CALLS_PER_TASK {
        server
            .search_web(
                strategy_id,
                Some(task_execution.a2a_task_id.clone()),
                params("example query"),
            )
            .await
            .expect("call within limit should succeed");
    }

    let err = server
        .search_web(
            strategy_id,
            Some(task_execution.a2a_task_id.clone()),
            params("example query"),
        )
        .await
        .expect_err("call beyond limit should fail");
    assert_eq!(
        (err, calls.lock().expect("record calls").len()),
        (
            rmcp::ErrorData::invalid_params(
                "execution budget limit was exceeded for search_web".to_string(),
                None,
            ),
            SEARCH_WEB_MAX_CALLS_PER_TASK as usize,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_keeps_call_count_when_request_fails(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let (client, calls) = client(vec![], true);
    let task_execution =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 20).await;
    let server = build_server(db).with_web_search_client(Some(client));

    let mut errors = Vec::new();
    for _ in 0..(SEARCH_WEB_MAX_CALLS_PER_TASK * 2) {
        errors.push(
            server
                .search_web(
                    strategy_id,
                    Some(task_execution.a2a_task_id.clone()),
                    params("example query"),
                )
                .await
                .expect_err("request should either fail upstream or exceed its budget"),
        );
    }
    assert_eq!(
        (errors, calls.lock().expect("record calls").len()),
        (
            (0..SEARCH_WEB_MAX_CALLS_PER_TASK)
                .map(|_| expected_denial(
                    TavilyCreditBudgetReason::ProviderRejected,
                    TavilyCreditBudgetScope::Provider,
                    Some(500),
                ))
                .chain((0..SEARCH_WEB_MAX_CALLS_PER_TASK).map(|_| {
                    rmcp::ErrorData::invalid_params(
                        "execution budget limit was exceeded for search_web",
                        None,
                    )
                }))
                .collect::<Vec<_>>(),
            SEARCH_WEB_MAX_CALLS_PER_TASK as usize,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_keeps_subagent_call_count_when_request_fails(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 4).await;
    let (client, calls) = client(vec![], true);
    let server = build_server(db).with_web_search_client(Some(client));

    let first_call = invoke_search(
        &server,
        strategy_id,
        &task_execution,
        Some((&Uuid::from_u128(601).to_string(), "researcher")),
    )
    .await;
    let retry = invoke_search(
        &server,
        strategy_id,
        &task_execution,
        Some((&Uuid::from_u128(601).to_string(), "researcher")),
    )
    .await;

    assert_eq!(
        (first_call, retry, calls.lock().expect("record calls").len()),
        (
            Err(expected_denial(
                TavilyCreditBudgetReason::ProviderRejected,
                TavilyCreditBudgetScope::Provider,
                Some(500),
            )),
            Err(rmcp::ErrorData::invalid_params(
                "execution budget limit was exceeded for researcher:search_web",
                None,
            )),
            1,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_enforces_subagent_and_task_limits(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 4).await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));

    let child_run_a = Uuid::from_u128(602).to_string();
    let child_run_b = Uuid::from_u128(603).to_string();
    let phase_call = invoke_search(&server, strategy_id, &task_execution, None).await;
    let first_subagent_call = invoke_search(
        &server,
        strategy_id,
        &task_execution,
        Some((&child_run_a, "researcher")),
    )
    .await;
    let repeated_subagent_call = invoke_search(
        &server,
        strategy_id,
        &task_execution,
        Some((&child_run_a, "researcher")),
    )
    .await;
    let other_subagent_call = invoke_search(
        &server,
        strategy_id,
        &task_execution,
        Some((&child_run_b, "researcher")),
    )
    .await;
    let second_phase_call = invoke_search(&server, strategy_id, &task_execution, None).await;
    let call_beyond_task_limit = invoke_search(&server, strategy_id, &task_execution, None).await;

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
            }], "retrieved_at": RETRIEVED_AT})),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            Err(rmcp::ErrorData::invalid_params(
                "execution budget limit was exceeded for researcher:search_web",
                None,
            )),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            Ok(json!({"results": [{
                "title": "Example article",
                "url": "https://example.invalid/article",
                "published_date": "2026-04-05",
                "snippet": "Example snippet",
            }], "retrieved_at": RETRIEVED_AT})),
            Err(rmcp::ErrorData::invalid_params(
                "execution budget limit was exceeded for search_web",
                None,
            )),
            4,
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_rejects_unknown_subagent_definition_key(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "example strategy").await;
    let task_execution =
        create_task_with_subagent_limits(&db, strategy_id, "researcher", "search_web", 1, 4).await;
    let (client, calls) = client(vec![example_result()], false);
    let server = build_server(db).with_web_search_client(Some(client));

    let result = invoke_search(
        &server,
        strategy_id,
        &task_execution,
        Some((&Uuid::from_u128(604).to_string(), "unknown-researcher")),
    )
    .await;

    assert_eq!(
        (result, calls.lock().expect("record calls").len()),
        (
            Err(rmcp::ErrorData::invalid_params(
                "execution budget request is invalid",
                None,
            )),
            0,
        ),
    );
}

async fn invoke_search(
    server: &super::test_server::StrategyServer,
    strategy_id: Uuid,
    task_execution: &TestExecutionContext,
    subagent: Option<(&str, &str)>,
) -> Result<serde_json::Value, rmcp::ErrorData> {
    let execution_step_id = subagent
        .map(|(execution_id, _)| execution_id.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-strategy-id",
        HeaderValue::from_str(&strategy_id.to_string()).expect("valid strategy ID"),
    );
    headers.insert(
        "x-execution-id",
        HeaderValue::from_str(&format!(
            "{}:{execution_step_id}",
            task_execution.a2a_task_id
        ))
        .expect("valid execution ID"),
    );
    headers.insert(
        "x-strategy-task-id",
        HeaderValue::from_str(&task_execution.task_id.to_string()).expect("valid strategy task ID"),
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
