#[cfg(test)]
mod tests {
    use super::super::{assert_response_eq, create_strategy};
    use crate::testing::create_test_server;

    #[backend_test_macros::database_test]
    async fn get_returns_null_when_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({ "amount_jpy": null, "effective_at": null })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        let expected = serde_json::json!({
            "amount_jpy": 1000000,
            "effective_at": "2020-01-01T00:00:00Z",
        });
        let put = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 1000000,
                "effective_at": "2020-01-01T00:00:00Z",
            }))
            .await;
        assert_response_eq(&put, axum::http::StatusCode::OK, Some(expected.clone()));

        let get = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        assert_response_eq(&get, axum::http::StatusCode::OK, Some(expected));
    }

    #[backend_test_macros::database_test]
    async fn put_without_effective_at_defaults_to_now(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;
        let before = chrono::Utc::now();

        let put = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({ "amount_jpy": 500000 }))
            .await;
        let mut body: serde_json::Value = put.json();
        let effective_at: chrono::DateTime<chrono::Utc> = body["effective_at"]
            .as_str()
            .expect("effective_at is a string")
            .parse()
            .expect("valid RFC3339 timestamp");

        body["effective_at"] = serde_json::json!("<effective_at>");
        assert_eq!(
            (put.status_code(), body, effective_at >= before),
            (
                axum::http::StatusCode::OK,
                serde_json::json!({
                    "amount_jpy": 500000,
                    "effective_at": "<effective_at>",
                }),
                true,
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_keeps_previous_history_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        let first = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 1000000,
                "effective_at": "2020-01-01T00:00:00Z",
            }))
            .await;
        assert_response_eq(
            &first,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "amount_jpy": 1000000,
                "effective_at": "2020-01-01T00:00:00Z",
            })),
        );
        let second = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            }))
            .await;
        assert_response_eq(
            &second,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            })),
        );

        let get = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        assert_response_eq(
            &get,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_ignores_future_history_and_returns_latest_effective_amount(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;
        let mut write_outputs = Vec::new();

        for (amount_jpy, effective_at) in [
            (100000, "2020-01-01T00:00:00Z"),
            (200000, "2020-06-01T00:00:00Z"),
            (900000, "2099-01-01T00:00:00Z"),
        ] {
            let response = server
                .put(&format!("/api/strategies/{id}/investable-amount"))
                .json(&serde_json::json!({
                    "amount_jpy": amount_jpy,
                    "effective_at": effective_at,
                }))
                .await;
            write_outputs.push((response.status_code(), response.json::<serde_json::Value>()));
        }

        let current = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;

        assert_eq!(
            (
                write_outputs,
                current.status_code(),
                current.json::<serde_json::Value>()
            ),
            (
                vec![
                    (
                        axum::http::StatusCode::OK,
                        serde_json::json!({ "amount_jpy": 100000, "effective_at": "2020-01-01T00:00:00Z" }),
                    ),
                    (
                        axum::http::StatusCode::OK,
                        serde_json::json!({ "amount_jpy": 200000, "effective_at": "2020-06-01T00:00:00Z" }),
                    ),
                    (
                        axum::http::StatusCode::OK,
                        serde_json::json!({ "amount_jpy": 900000, "effective_at": "2099-01-01T00:00:00Z" }),
                    ),
                ],
                axum::http::StatusCode::OK,
                serde_json::json!({
                    "amount_jpy": 200000,
                    "effective_at": "2020-06-01T00:00:00Z",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_404_for_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/investable-amount")
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(
                serde_json::json!({ "error": "strategy 00000000-0000-0000-0000-000000000000 not found" }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_404_for_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .put("/api/strategies/00000000-0000-0000-0000-000000000000/investable-amount")
            .json(&serde_json::json!({ "amount_jpy": 1000000 }))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(
                serde_json::json!({ "error": "strategy 00000000-0000-0000-0000-000000000000 not found" }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_400_for_negative_amount(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({ "amount_jpy": -1 }))
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::BAD_REQUEST,
            Some(serde_json::json!({ "error": "amount_jpy must be non-negative" })),
        );
    }
}
