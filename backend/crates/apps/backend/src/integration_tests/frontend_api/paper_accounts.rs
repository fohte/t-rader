#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_agent_config, create_strategy};
    use crate::testing::{create_test_server, insert_test_stock};
    use axum::http::StatusCode;
    use serde_json::{Value, json};
    use uuid::Uuid;

    async fn server_with_account_references(
        db: gateway_postgres::DatabaseHandle,
    ) -> (axum_test::TestServer, String) {
        insert_test_stock(&db, "DEMO-BENCHMARK", "Demo benchmark").await;
        let server = create_test_server(db).await;
        let strategy_id = create_strategy(&server, "sample-strategy").await;
        create_agent_config(&server, "example-purpose").await;
        (server, strategy_id)
    }

    fn account_request(
        strategy_id: &str,
        purpose: &str,
        name: &str,
        benchmark_stock_id: Option<&str>,
    ) -> Value {
        json!({
            "strategy_id": strategy_id,
            "purpose": purpose,
            "name": name,
            "initial_cash_jpy": 1500000,
            "benchmark_stock_id": benchmark_stock_id,
            "started_on": "2026-01-02",
        })
    }

    fn normalize_account(mut value: Value) -> Value {
        if let Some(accounts) = value.as_array_mut() {
            for account in accounts {
                if let Some(id) = account.get_mut("id") {
                    *id = json!("<id>");
                }
            }
        } else if let Some(id) = value.get_mut("id") {
            *id = json!("<id>");
        }
        value
    }

    async fn create_account(
        server: &axum_test::TestServer,
        request: Value,
    ) -> axum_test::TestResponse {
        server.post("/api/paper-accounts").json(&request).await
    }

    #[backend_test_macros::database_test]
    async fn create_returns_the_paper_account(db: gateway_postgres::DatabaseHandle) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let response = create_account(
            &server,
            account_request(
                &strategy_id,
                "example-purpose",
                "sample-account",
                Some("DEMO-BENCHMARK"),
            ),
        )
        .await;
        let body = normalize_account(response.json());

        assert_eq!(
            (response.status_code(), body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "name": "sample-account",
                    "strategy_id": strategy_id,
                    "purpose": "example-purpose",
                    "initial_cash_jpy": 1500000,
                    "benchmark_stock_id": "DEMO-BENCHMARK",
                    "started_on": "2026-01-02",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_returns_created_paper_accounts(db: gateway_postgres::DatabaseHandle) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let _ = create_account(
            &server,
            account_request(
                &strategy_id,
                "example-purpose",
                "sample-account",
                Some("DEMO-BENCHMARK"),
            ),
        )
        .await;

        let response = server.get("/api/paper-accounts").await;
        let body = normalize_account(response.json());

        assert_eq!(
            (response.status_code(), body),
            (
                StatusCode::OK,
                json!([{
                    "id": "<id>",
                    "name": "sample-account",
                    "strategy_id": strategy_id,
                    "purpose": "example-purpose",
                    "initial_cash_jpy": 1500000,
                    "benchmark_stock_id": "DEMO-BENCHMARK",
                    "started_on": "2026-01-02",
                }]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_missing_strategy_purpose_and_benchmark(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let missing_strategy_id = Uuid::new_v4().to_string();
        let requests = [
            account_request(
                &missing_strategy_id,
                "example-purpose",
                "missing-strategy-account",
                None,
            ),
            account_request(
                &strategy_id,
                "missing-purpose",
                "missing-purpose-account",
                None,
            ),
            account_request(
                &strategy_id,
                "example-purpose",
                "missing-benchmark-account",
                Some("DEMO-MISSING"),
            ),
        ];

        let mut actual = Vec::new();
        for request in requests {
            let response = create_account(&server, request).await;
            actual.push((response.status_code(), response.json::<Value>()));
        }

        assert_eq!(
            actual,
            vec![
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "error": "referenced resource does not exist" }),
                );
                3
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_a_duplicate_strategy_purpose_pair(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, strategy_id) = server_with_account_references(db).await;
        let first = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "first-account", None),
        )
        .await;
        first.assert_status(StatusCode::CREATED);

        let duplicate = create_account(
            &server,
            account_request(&strategy_id, "example-purpose", "second-account", None),
        )
        .await;

        assert_response_eq(
            &duplicate,
            StatusCode::CONFLICT,
            Some(json!({ "error": "resource already exists" })),
        );
    }
}
