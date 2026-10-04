#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
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
        let mut note = response.json::<Value>();
        let note_id = note["id"].as_str().expect("note id").to_string();
        let version_id = note["version_id"].as_str().expect("version id").to_string();
        let version_no = note["version_no"].as_i64().expect("version number");
        note["id"] = json!("<id>");
        note["version_id"] = json!("<version_id>");
        note["created_at"] = json!("<created_at>");
        note["updated_at"] = json!("<updated_at>");
        assert_eq!(
            (response.status_code(), note),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "title": title,
                    "body_md": body_md,
                    "frontmatter_json": {},
                    "resolved_price_references_json": {},
                    "graphs_json": [],
                    "tags": [],
                    "kind": null,
                    "status": "unread",
                    "trigger": null,
                    "trigger_label": null,
                    "created_by_kind": "llm",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        (note_id, version_id, version_no)
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
        let plain_comment = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": plain_version_id,
                "body": "sample comment",
            }))
            .await;
        let mut plain_comment_body = plain_comment.json::<Value>();
        plain_comment_body["id"] = json!("<id>");
        plain_comment_body["created_at"] = json!("<created_at>");
        assert_eq!(
            (plain_comment.status_code(), plain_comment_body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_kind": "note_version",
                    "target_id": plain_version_id,
                    "parent_id": null,
                    "body": "sample comment",
                    "author_kind": "human",
                    "author_label": "user",
                    "created_at": "<created_at>",
                    "resolved": false,
                    "anchor_text": null,
                    "start_line": null,
                    "end_line": null,
                    "anchor_side": null,
                }),
            ),
        );
        let anchored_comment = server
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
            .await;
        let mut anchored_comment_body = anchored_comment.json::<Value>();
        anchored_comment_body["id"] = json!("<id>");
        anchored_comment_body["created_at"] = json!("<created_at>");
        assert_eq!(
            (anchored_comment.status_code(), anchored_comment_body),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_kind": "note_version",
                    "target_id": anchored_version_id,
                    "parent_id": null,
                    "body": "sample line comment",
                    "author_kind": "human",
                    "author_label": "user",
                    "created_at": "<created_at>",
                    "resolved": false,
                    "anchor_text": "line two",
                    "start_line": 2,
                    "end_line": 2,
                    "anchor_side": "new",
                }),
            ),
        );

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
        assert_response_eq(
            &plain_rejection,
            StatusCode::BAD_REQUEST,
            Some(json!({
                "error": "a rejection reason is required when no line comments are attached"
            })),
        );
        assert_eq!(
            (
                anchored_rejection.status_code(),
                normalize_note_version(anchored_rejection.json()),
            ),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "note_id": "<note_id>",
                    "version_no": anchored_version_no,
                    "title": "anchored sample note",
                    "body_md": anchored_body,
                    "frontmatter_json": {},
                    "resolved_price_references_json": {},
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
