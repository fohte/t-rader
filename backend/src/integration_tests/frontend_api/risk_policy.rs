#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use crate::testing::{create_test_server, insert_test_group};

    #[backend_test_macros::database_test]
    async fn get_returns_null_when_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let res = server.get("/api/account/risk-policy").await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({ "max_group_ratios": [] })),
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
        assert_response_eq(&put, axum::http::StatusCode::OK, Some(expected.clone()));

        let get = server.get("/api/account/risk-policy").await;
        assert_response_eq(&get, axum::http::StatusCode::OK, Some(expected));
    }

    #[backend_test_macros::database_test]
    async fn put_multiple_times_updates_to_latest_value(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        let server = create_test_server(db).await;

        let first = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            }))
            .await;
        assert_response_eq(
            &first,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            })),
        );
        let second = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            }))
            .await;
        assert_response_eq(
            &second,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            })),
        );

        let get = server.get("/api/account/risk-policy").await;
        assert_response_eq(
            &get,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_empty_list_clears_limit(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        let server = create_test_server(db).await;

        let initial = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            }))
            .await;
        assert_response_eq(
            &initial,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            })),
        );
        let cleared = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_group_ratios": [] }))
            .await;
        assert_response_eq(
            &cleared,
            axum::http::StatusCode::OK,
            Some(serde_json::json!({ "max_group_ratios": [] })),
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
            assert_response_eq(
                &res,
                axum::http::StatusCode::BAD_REQUEST,
                Some(serde_json::json!({
                    "error": "ratio must be greater than 0 and less than or equal to 1"
                })),
            );
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

        assert_response_eq(
            &res,
            axum::http::StatusCode::BAD_REQUEST,
            Some(serde_json::json!({ "error": "unknown axis: unknown-axis" })),
        );
    }
}
