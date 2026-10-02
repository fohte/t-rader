#[cfg(test)]
mod tests {
    use super::super::{
        assert_response_eq, create_strategy,
        create_strategy_with_description as create_strategy_request, normalize_strategy,
    };
    use crate::testing::create_test_server;
    use serde_json::{Value, json};

    async fn create_strategy_with_description(
        db: gateway_postgres::DatabaseHandle,
    ) -> (axum_test::TestServer, String) {
        let server = create_test_server(db).await;
        let id = create_strategy_request(&server, "strategy", Some("initial")).await;
        (server, id)
    }

    #[backend_test_macros::database_test]
    async fn create_and_list_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": "長期投資" }))
            .await;
        let id = res.json::<Value>()["id"].as_str().unwrap().to_string();
        assert_eq!(
            (res.status_code(), normalize_strategy(res.json())),
            (
                axum::http::StatusCode::CREATED,
                json!({
                    "id": id,
                    "name": "長期投資",
                    "description": null,
                    "sort_order": 0,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );

        let list = server.get("/api/strategies").await;
        assert_eq!(
            (
                list.status_code(),
                list.json::<Vec<Value>>()
                    .into_iter()
                    .map(normalize_strategy)
                    .collect::<Vec<_>>()
            ),
            (
                axum::http::StatusCode::OK,
                vec![json!({
                    "id": id,
                    "name": "長期投資",
                    "description": null,
                    "sort_order": 0,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_description_omitted_keeps_existing_value(db: gateway_postgres::DatabaseHandle) {
        let (server, id) = create_strategy_with_description(db).await;
        let updated = server
            .patch(&format!("/api/strategies/{id}"))
            .json(&json!({}))
            .await;
        assert_eq!(
            (updated.status_code(), normalize_strategy(updated.json())),
            (
                axum::http::StatusCode::OK,
                json!({
                    "id": id,
                    "name": "strategy",
                    "description": "initial",
                    "sort_order": 0,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_description_null_clears_existing_value(db: gateway_postgres::DatabaseHandle) {
        let (server, id) = create_strategy_with_description(db).await;
        let updated = server
            .patch(&format!("/api/strategies/{id}"))
            .json(&json!({ "description": null }))
            .await;
        assert_eq!(
            (updated.status_code(), normalize_strategy(updated.json())),
            (
                axum::http::StatusCode::OK,
                json!({
                    "id": id,
                    "name": "strategy",
                    "description": null,
                    "sort_order": 0,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_description_value_replaces_existing_value(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let (server, id) = create_strategy_with_description(db).await;
        let updated = server
            .patch(&format!("/api/strategies/{id}"))
            .json(&json!({ "description": "replacement" }))
            .await;
        assert_eq!(
            (updated.status_code(), normalize_strategy(updated.json())),
            (
                axum::http::StatusCode::OK,
                json!({
                    "id": id,
                    "name": "strategy",
                    "description": "replacement",
                    "sort_order": 0,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_strategy_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000")
            .await;
        assert_response_eq(
            &res,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({ "error": "strategy 00000000-0000-0000-0000-000000000000 not found" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_removes_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "to-delete").await;

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        assert_response_eq(&deleted, axum::http::StatusCode::NO_CONTENT, None);

        let get = server.get(&format!("/api/strategies/{id}")).await;
        assert_response_eq(
            &get,
            axum::http::StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("strategy {id} not found") })),
        );
    }
}
