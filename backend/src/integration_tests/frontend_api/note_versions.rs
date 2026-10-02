#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Value, json};

    use crate::testing::create_test_server;

    fn normalize_note_version(mut value: Value) -> Value {
        for key in ["id", "note_id", "created_at", "reviewed_at"] {
            if let Some(value) = value.as_object_mut().and_then(|object| object.get_mut(key)) {
                *value = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    async fn create_pending_note(
        server: &axum_test::TestServer,
        title: &str,
        body_md: &str,
    ) -> (String, String, i64) {
        let response = server
            .post("/api/notes")
            .json(&json!({
                "title": title,
                "body_md": body_md,
                "created_by_kind": "llm",
                "status": "unread",
            }))
            .await;
        response.assert_status(StatusCode::CREATED);
        let note = response.json::<Value>();
        (
            note["id"].as_str().expect("note id").to_string(),
            note["version_id"].as_str().expect("version id").to_string(),
            note["version_no"].as_i64().expect("version number"),
        )
    }

    #[backend_test_macros::database_test]
    async fn reject_note_version_counts_only_line_anchored_comments(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let (plain_note_id, plain_version_id, plain_version_no) =
            create_pending_note(&server, "sample note", "sample body").await;
        let anchored_body = indoc::indoc! {"
            line one
            line two"};
        let (anchored_note_id, anchored_version_id, anchored_version_no) =
            create_pending_note(&server, "anchored sample note", anchored_body).await;
        server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": plain_version_id,
                "body": "sample comment",
            }))
            .await
            .assert_status(StatusCode::CREATED);
        server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": anchored_version_id,
                "body": "sample line comment",
                "anchor_text": "line two",
                "anchor_side": "new",
                "start_line": 2,
                "end_line": 2,
            }))
            .await
            .assert_status(StatusCode::CREATED);

        let plain_rejection = server
            .post(&format!(
                "/api/notes/{plain_note_id}/versions/{plain_version_no}/reject"
            ))
            .json(&json!({}))
            .await;
        let anchored_rejection = server
            .post(&format!(
                "/api/notes/{anchored_note_id}/versions/{anchored_version_no}/reject"
            ))
            .json(&json!({}))
            .await;
        assert_eq!(
            (
                plain_rejection.status_code(),
                plain_rejection.json::<Value>(),
                anchored_rejection.status_code(),
                normalize_note_version(anchored_rejection.json()),
            ),
            (
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "a rejection reason is required when no line comments are attached"
                }),
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "note_id": "<note_id>",
                    "version_no": anchored_version_no,
                    "title": "anchored sample note",
                    "body_md": anchored_body,
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "status": "rejected",
                    "is_current": true,
                    "change_reason": null,
                    "created_by_kind": "llm",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "reviewed_at": "<reviewed_at>",
                }),
            ),
        );
    }
}
