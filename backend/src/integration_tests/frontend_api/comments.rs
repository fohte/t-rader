#[cfg(test)]
mod tests {
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "target_id", "created_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    fn normalize_comment_list(mut value: Value) -> Value {
        if let Some(comments) = value.as_array_mut() {
            for comment in comments {
                for key in ["id", "target_id", "created_at"] {
                    if let Some(value) = comment
                        .as_object_mut()
                        .and_then(|object| object.get_mut(key))
                    {
                        *value = Value::String(format!("<{key}>"));
                    }
                }
                if let Some(parent_id) = comment
                    .as_object_mut()
                    .and_then(|object| object.get_mut("parent_id"))
                    .filter(|parent_id| !parent_id.is_null())
                {
                    *parent_id = Value::String("<id>".into());
                }
            }
        }
        value
    }

    async fn create_note_comment(server: &axum_test::TestServer) -> Value {
        let strategy_id = crate::testing::create_strategy(server, "s").await;
        let note_id = create_note(server, &strategy_id, "body").await;
        let version_id = first_note_version_id(server, &note_id).await;
        server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": version_id,
                "body": "fix this",
            }))
            .await
            .json()
    }

    #[backend_test_macros::database_test]
    async fn list_comments_returns_target_threads_in_creation_order(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let root = create_note_comment(&server).await;
        let reply = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": root["target_kind"],
                "target_id": root["target_id"],
                "parent_id": root["id"],
                "body": "sample reply",
            }))
            .await;
        let list = server
            .get(&format!(
                "/api/comments?target_kind=note_version&target_id={}",
                root["target_id"].as_str().expect("target id"),
            ))
            .await;
        assert_eq!(
            (
                reply.status_code(),
                list.status_code(),
                normalize_comment_list(list.json()),
            ),
            (
                StatusCode::CREATED,
                StatusCode::OK,
                json!([
                    {
                        "id": "<id>",
                        "target_kind": "note_version",
                        "target_id": "<target_id>",
                        "parent_id": null,
                        "body": "fix this",
                        "author_kind": "human",
                        "author_label": "user",
                        "resolved": false,
                        "created_at": "<created_at>",
                        "anchor_text": null,
                        "anchor_side": null,
                        "start_line": null,
                        "end_line": null,
                    },
                    {
                        "id": "<id>",
                        "target_kind": "note_version",
                        "target_id": "<target_id>",
                        "parent_id": "<id>",
                        "body": "sample reply",
                        "author_kind": "human",
                        "author_label": "user",
                        "resolved": false,
                        "created_at": "<created_at>",
                        "anchor_text": null,
                        "anchor_side": null,
                        "start_line": null,
                        "end_line": null,
                    },
                ]),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_comment_sets_resolved(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let created = create_note_comment(&server).await;
        let id = created["id"].as_str().expect("id");

        let res = server
            .patch(&format!("/api/comments/{id}"))
            .json(&json!({ "resolved": true }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note_version",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this",
                "author_kind": "human",
                "author_label": "user",
                "resolved": true,
                "created_at": "<created_at>",
                "anchor_text": null,
                "anchor_side": null,
                "start_line": null,
                "end_line": null,
            }),
        );

        let res = server
            .patch(&format!("/api/comments/{id}"))
            .json(&json!({ "resolved": false }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note_version",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": null,
                "anchor_side": null,
                "start_line": null,
                "end_line": null,
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_comment_missing_id_is_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .patch(&format!("/api/comments/{}", uuid::Uuid::new_v4()))
            .json(&json!({ "resolved": true }))
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn create_comment_rejects_reply_to_reply(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let root = create_note_comment(&server).await;
        let reply_response = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": root["target_kind"],
                "target_id": root["target_id"],
                "parent_id": root["id"],
                "body": "first reply",
            }))
            .await;
        reply_response.assert_status(StatusCode::CREATED);
        let reply = reply_response.json::<Value>();

        let nested_response = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": root["target_kind"],
                "target_id": root["target_id"],
                "parent_id": reply["id"],
                "body": "nested reply",
            }))
            .await;
        assert_eq!(
            (
                nested_response.status_code(),
                nested_response.json::<Value>()
            ),
            (
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "cannot reply to a reply; parent_id must reference a top-level comment",
                }),
            ),
        );
    }

    async fn create_note(
        server: &axum_test::TestServer,
        strategy_id: &str,
        body_md: &str,
    ) -> String {
        let created = server
            .post("/api/notes")
            .json(&json!({
                "strategy_id": strategy_id,
                "title": "note",
                "body_md": body_md,
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        created.json::<Value>()["id"]
            .as_str()
            .expect("id")
            .to_string()
    }

    async fn first_note_version_id(server: &axum_test::TestServer, note_id: &str) -> String {
        let versions = server.get(&format!("/api/notes/{note_id}/versions")).await;
        versions.assert_status_ok();
        versions.json::<Value>()[0]["id"]
            .as_str()
            .expect("version id")
            .to_string()
    }

    async fn create_annotation(server: &axum_test::TestServer, strategy_id: &str) -> String {
        let created = server
            .post("/api/annotations")
            .json(&json!({
                "strategy_id": strategy_id,
                "target_symbol": "7203",
                "target_kind": "observation",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        created.assert_status(StatusCode::CREATED);
        created.json::<Value>()["id"]
            .as_str()
            .expect("id")
            .to_string()
    }

    #[backend_test_macros::database_test]
    async fn create_comment_with_line_anchor_stores_explicit_lines(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let strategy_id = crate::testing::create_strategy(&server, "s").await;
        let note_id = create_note(
            &server,
            &strategy_id,
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;
        let version_id = first_note_version_id(&server, &note_id).await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": version_id,
                "body": "fix this line",
                "anchor_text": "line two",
                "anchor_side": "new",
                "start_line": 2,
                "end_line": 2,
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note_version",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this line",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "line two",
                "anchor_side": "new",
                "start_line": 2,
                "end_line": 2,
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_comment_keeps_quote_with_explicit_line_anchor(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let strategy_id = crate::testing::create_strategy(&server, "s").await;
        let note_id = create_note(
            &server,
            &strategy_id,
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;
        let version_id = first_note_version_id(&server, &note_id).await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": version_id,
                "body": "fix this line",
                "anchor_text": "line that no longer exists",
                "anchor_side": "new",
                "start_line": 3,
                "end_line": 3,
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "note_version",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this line",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "line that no longer exists",
                "anchor_side": "new",
                "start_line": 3,
                "end_line": 3,
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_comment_on_annotation_with_anchor_text_saves_text_without_line_numbers(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let strategy_id = crate::testing::create_strategy(&server, "s").await;
        let annotation_id = create_annotation(&server, &strategy_id).await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "annotation",
                "target_id": annotation_id,
                "body": "fix this",
                "anchor_text": "some selected text",
            }))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "target_kind": "annotation",
                "target_id": "<target_id>",
                "parent_id": null,
                "body": "fix this",
                "author_kind": "human",
                "author_label": "user",
                "resolved": false,
                "created_at": "<created_at>",
                "anchor_text": "some selected text",
                "anchor_side": null,
                "start_line": null,
                "end_line": null,
            }),
        );
    }
}
