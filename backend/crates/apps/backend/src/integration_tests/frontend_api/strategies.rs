#[cfg(test)]
mod tests {
    use super::super::{
        assert_response_eq, create_strategy,
        create_strategy_with_description as create_strategy_request, normalize_annotation_response,
        normalize_note_response, normalize_strategy,
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

    #[backend_test_macros::database_test]
    async fn delete_strategy_preserves_notes_and_annotations(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let strategy_id = create_strategy(&server, "temporary-strategy").await;
        let note_response = server
            .post("/api/notes")
            .json(&json!({ "title": "sample note", "body_md": "sample body" }))
            .await;
        let note_id = note_response.json::<Value>()["id"]
            .as_str()
            .expect("note id")
            .to_string();
        let annotation_response = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "sample-code",
                "target_kind": "sample-kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "sample annotation",
            }))
            .await;
        let annotation_id = annotation_response.json::<Value>()["id"]
            .as_str()
            .expect("annotation id")
            .to_string();

        let deleted = server
            .delete(&format!("/api/strategies/{strategy_id}"))
            .await;
        let saved_note = server.get(&format!("/api/notes/{note_id}")).await;
        let saved_annotation = server
            .get(&format!("/api/annotations/{annotation_id}"))
            .await;

        assert_eq!(
            (
                deleted.status_code(),
                saved_note.status_code(),
                normalize_note_response(saved_note.json()),
                saved_annotation.status_code(),
                normalize_annotation_response(saved_annotation.json()),
            ),
            (
                axum::http::StatusCode::NO_CONTENT,
                axum::http::StatusCode::OK,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "title": "sample note",
                    "body_md": "sample body",
                    "frontmatter_json": {},
                    "resolved_price_references_json": {},
                    "tags": [],
                    "kind": null,
                    "status": "approved",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "graphs_json": [],
                    "execution_id": null,
                }),
                axum::http::StatusCode::OK,
                json!({
                    "id": "<id>",
                    "target_symbol": "sample-code",
                    "target_kind": "sample-kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "sample annotation",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "timestamp_start": null,
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );
    }
}
