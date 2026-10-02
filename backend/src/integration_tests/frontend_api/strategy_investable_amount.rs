#[cfg(test)]
mod tests {
    use crate::testing::{create_strategy, create_test_server};

    #[backend_test_macros::database_test]
    async fn get_returns_null_when_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "amount_jpy": null, "effective_at": null }),
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
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
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
        put.assert_status_ok();
        let body: serde_json::Value = put.json();
        let effective_at: chrono::DateTime<chrono::Utc> = body["effective_at"]
            .as_str()
            .expect("effective_at is a string")
            .parse()
            .expect("valid RFC3339 timestamp");

        assert_eq!(body["amount_jpy"], serde_json::json!(500000));
        assert!(effective_at >= before);
    }

    #[backend_test_macros::database_test]
    async fn put_keeps_previous_history_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 1000000,
                "effective_at": "2020-01-01T00:00:00Z",
            }))
            .await
            .assert_status_ok();
        server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            }))
            .await
            .assert_status_ok();

        let get = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;
        get.assert_status_ok();
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({
                "amount_jpy": 2000000,
                "effective_at": "2020-06-01T00:00:00Z",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_ignores_future_history_and_returns_latest_effective_amount(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;
        let mut write_statuses = Vec::new();

        for (amount_jpy, effective_at) in [
            (100000, "2020-01-01T00:00:00Z"),
            (200000, "2020-06-01T00:00:00Z"),
            (900000, "2099-01-01T00:00:00Z"),
        ] {
            write_statuses.push(
                server
                    .put(&format!("/api/strategies/{id}/investable-amount"))
                    .json(&serde_json::json!({
                        "amount_jpy": amount_jpy,
                        "effective_at": effective_at,
                    }))
                    .await
                    .status_code(),
            );
        }

        let current = server
            .get(&format!("/api/strategies/{id}/investable-amount"))
            .await;

        assert_eq!(
            (
                write_statuses,
                current.status_code(),
                current.json::<serde_json::Value>(),
            ),
            (
                vec![
                    axum::http::StatusCode::OK,
                    axum::http::StatusCode::OK,
                    axum::http::StatusCode::OK,
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
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn put_404_for_unknown_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .put("/api/strategies/00000000-0000-0000-0000-000000000000/investable-amount")
            .json(&serde_json::json!({ "amount_jpy": 1000000 }))
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn put_400_for_negative_amount(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "s").await;

        let res = server
            .put(&format!("/api/strategies/{id}/investable-amount"))
            .json(&serde_json::json!({ "amount_jpy": -1 }))
            .await;
        res.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }
}
