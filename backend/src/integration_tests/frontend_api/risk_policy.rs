#[cfg(test)]
mod tests {
    use crate::testing::{create_test_server, insert_test_group};

    #[backend_test_macros::database_test]
    async fn get_returns_null_when_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let res = server.get("/api/account/risk-policy").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "max_group_ratios": [] }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_round_trips(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        insert_test_group(
            &db,
            "another-sample-axis",
            "another-sample-group",
            "Another sample group",
        )
        .await;
        let server = create_test_server(db).await;

        let expected = serde_json::json!({
            "max_group_ratios": [
                { "axis": "sample-axis", "ratio": 0.3 },
                { "axis": "another-sample-axis", "ratio": 0.2 },
            ]
        });
        let put = server.put("/api/account/risk-policy").json(&expected).await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server.get("/api/account/risk-policy").await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
    }

    #[backend_test_macros::database_test]
    async fn put_multiple_times_updates_to_latest_value(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        let server = create_test_server(db).await;

        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            }))
            .await
            .assert_status_ok();
        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            }))
            .await
            .assert_status_ok();

        let get = server.get("/api/account/risk-policy").await;
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_empty_list_clears_limit(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        let server = create_test_server(db).await;

        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            }))
            .await
            .assert_status_ok();
        let cleared = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_group_ratios": [] }))
            .await;
        cleared.assert_status_ok();
        assert_eq!(
            cleared.json::<serde_json::Value>(),
            serde_json::json!({ "max_group_ratios": [] }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_400_for_out_of_range_ratio(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        for invalid in [serde_json::json!(0), serde_json::json!(1.5)] {
            let res = server
                .put("/api/account/risk-policy")
                .json(&serde_json::json!({
                    "max_group_ratios": [
                        { "axis": "sample-axis", "ratio": 0.3 },
                        { "axis": "another-sample-axis", "ratio": invalid },
                    ]
                }))
                .await;
            res.assert_status(axum::http::StatusCode::BAD_REQUEST);
        }
    }

    #[backend_test_macros::database_test]
    async fn put_400_for_unknown_axis(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "unknown-axis", "ratio": 0.3 }]
            }))
            .await;

        res.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }
}
