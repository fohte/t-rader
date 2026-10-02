use indoc::indoc;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::SEARCH_WEB_MAX_CALLS_PER_TASK;
use super::dto::SearchWebParams;
use super::tests_common::{build_server, insert_strategy};
use crate::services::litellm_client::LiteLlmClient;

fn params(query: &str) -> SearchWebParams {
    SearchWebParams {
        query: query.into(),
    }
}

fn sse_body(content: &str) -> String {
    format!(
        indoc! {"
                data: {}

                data: [DONE]

            "},
        json!({"choices": [{"delta": {"content": content}}]})
    )
}

#[backend_test_macros::database_test]
async fn search_web_requires_litellm_client(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "test").await;
    let server = build_server(db);
    let err = server
        .search_web(
            strategy_id,
            None,
            "example-model-search".to_string(),
            params("半導体 関連ニュース"),
        )
        .await
        .expect_err("expected internal error");
    assert_eq!(
        err,
        rmcp::ErrorData::internal_error("litellm client is not configured", None),
    );
}

#[backend_test_macros::database_test]
async fn search_web_rejects_empty_query(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "test").await;
    let server = build_server(db);
    let err = server
        .search_web(
            strategy_id,
            None,
            "example-model-search".to_string(),
            params("   "),
        )
        .await
        .expect_err("expected invalid params");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params("query must not be empty", None),
    );
}

#[backend_test_macros::database_test]
async fn search_web_returns_text_and_citations(db: gateway_postgres::DatabaseHandle) {
    let litellm = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body("半導体銘柄が上昇")))
        .mount(&litellm)
        .await;

    let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
    let strategy_id = insert_strategy(&db, "test").await;
    let server = build_server(db).with_litellm_client(Some(std::sync::Arc::new(client)));

    let out = server
        .search_web(
            strategy_id,
            None,
            "example-model-search".to_string(),
            params("半導体 関連ニュース"),
        )
        .await
        .expect("search_web");
    let requests = litellm
        .received_requests()
        .await
        .expect("recorded requests");
    let body: serde_json::Value = requests[0].body_json().expect("parse request body");
    assert_eq!(
        (out.as_json().clone(), body),
        (
            json!({"text": "半導体銘柄が上昇", "citations": []}),
            json!({
                "model": "example-model-search",
                "messages": [{
                    "role": "user",
                    "content": [{"type": "text", "text": "半導体 関連ニュース"}],
                }],
                "stream": true,
                "web_search_options": {},
                "allowed_openai_params": ["web_search_options"],
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn search_web_enforces_per_task_call_limit(db: gateway_postgres::DatabaseHandle) {
    let litellm = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse_body("ok")))
        .mount(&litellm)
        .await;

    let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
    let strategy_id = insert_strategy(&db, "test").await;
    let server = build_server(db).with_litellm_client(Some(std::sync::Arc::new(client)));
    let task_execution_id = format!("task-{}", Uuid::new_v4());

    for _ in 0..SEARCH_WEB_MAX_CALLS_PER_TASK {
        server
            .search_web(
                strategy_id,
                Some(task_execution_id.clone()),
                "example-model-search".to_string(),
                params("query"),
            )
            .await
            .expect("call within limit should succeed");
    }

    let err = server
        .search_web(
            strategy_id,
            Some(task_execution_id.clone()),
            "example-model-search".to_string(),
            params("query"),
        )
        .await
        .expect_err("call beyond limit should fail");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params(
            format!(
                "search_web call limit ({SEARCH_WEB_MAX_CALLS_PER_TASK}) exceeded for this task execution"
            ),
            None,
        ),
    );

    let requests = litellm
        .received_requests()
        .await
        .expect("recorded requests");
    assert_eq!(requests.len(), SEARCH_WEB_MAX_CALLS_PER_TASK as usize);
}

#[backend_test_macros::database_test]
async fn search_web_releases_call_count_reservation_when_llm_request_fails(
    db: gateway_postgres::DatabaseHandle,
) {
    let litellm = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream error"))
        .mount(&litellm)
        .await;

    let client = LiteLlmClient::new(&litellm.uri(), None).expect("build client");
    let strategy_id = insert_strategy(&db, "test").await;
    let server = build_server(db).with_litellm_client(Some(std::sync::Arc::new(client)));
    let task_execution_id = format!("task-{}", Uuid::new_v4());

    // 予約したカウントが都度解放されなければ、SEARCH_WEB_MAX_CALLS_PER_TASK 回目以降は
    // 呼び出し上限エラー (INVALID_PARAMS) に化けてしまう。上限の 2 倍失敗させても常に
    // upstream の失敗 (INTERNAL_ERROR) のまま伝わることを確認する。
    for _ in 0..(SEARCH_WEB_MAX_CALLS_PER_TASK * 2) {
        let err = server
            .search_web(
                strategy_id,
                Some(task_execution_id.clone()),
                "example-model-search".to_string(),
                params("query"),
            )
            .await
            .expect_err("upstream failure should propagate");
        assert_eq!(
            err,
            rmcp::ErrorData::internal_error("litellm api error (status 500): upstream error", None,),
        );
    }
}
