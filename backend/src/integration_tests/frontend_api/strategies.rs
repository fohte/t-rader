#[cfg(test)]
mod tests {
    use crate::testing::{create_strategy, create_test_server};
    use serde_json::{Value, json};

    fn normalize_strategy(mut value: Value) -> Value {
        for key in ["created_at", "updated_at"] {
            if let Some(field) = value.get_mut(key) {
                *field = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    async fn create_strategy_with_description(
        db: gateway_postgres::DatabaseHandle,
    ) -> (axum_test::TestServer, String) {
        let server = create_test_server(db).await;
        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": "strategy", "description": "initial" }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        let id = created.json::<Value>()["id"].as_str().unwrap().to_string();
        (server, id)
    }

    #[backend_test_macros::database_test]
    async fn create_and_list_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": "長期投資" }))
            .await;
        res.assert_status(axum::http::StatusCode::CREATED);

        let list = server.get("/api/strategies").await;
        list.assert_status_ok();
        let body: Vec<serde_json::Value> = list.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["name"], "長期投資");
    }

    #[backend_test_macros::database_test]
    async fn update_description_omitted_keeps_existing_value(db: gateway_postgres::DatabaseHandle) {
        let (server, id) = create_strategy_with_description(db).await;
        let updated = server
            .patch(&format!("/api/strategies/{id}"))
            .json(&json!({}))
            .await;
        updated.assert_status_ok();
        assert_eq!(
            normalize_strategy(updated.json()),
            json!({
                "id": id,
                "name": "strategy",
                "description": "initial",
                "sort_order": 0,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_description_null_clears_existing_value(db: gateway_postgres::DatabaseHandle) {
        let (server, id) = create_strategy_with_description(db).await;
        let updated = server
            .patch(&format!("/api/strategies/{id}"))
            .json(&json!({ "description": null }))
            .await;
        updated.assert_status_ok();
        assert_eq!(
            normalize_strategy(updated.json()),
            json!({
                "id": id,
                "name": "strategy",
                "description": null,
                "sort_order": 0,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
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
        updated.assert_status_ok();
        assert_eq!(
            normalize_strategy(updated.json()),
            json!({
                "id": id,
                "name": "strategy",
                "description": "replacement",
                "sort_order": 0,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_strategy_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_removes_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "to-delete").await;

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        deleted.assert_status(axum::http::StatusCode::NO_CONTENT);

        let get = server.get(&format!("/api/strategies/{id}")).await;
        get.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
