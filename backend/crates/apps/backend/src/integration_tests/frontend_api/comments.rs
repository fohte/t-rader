#[cfg(test)]
mod tests {
    use super::super::{
        assert_response_eq, normalize_annotation_response, normalize_note_response,
    };
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

    fn expected_comment(
        target_kind: &str,
        parent_id: Option<&str>,
        body: &str,
        anchor_text: Option<&str>,
        anchor_side: Option<&str>,
        start_line: Option<i64>,
        end_line: Option<i64>,
    ) -> Value {
        json!({
            "id": "<id>",
            "target_kind": target_kind,
            "target_id": "<target_id>",
            "parent_id": parent_id,
            "body": body,
            "author_kind": "human",
            "author_label": "user",
            "resolved": false,
            "created_at": "<created_at>",
            "anchor_text": anchor_text,
            "anchor_side": anchor_side,
            "start_line": start_line,
            "end_line": end_line,
        })
    }

    fn normalize_note_version_list(mut value: Value) -> Value {
        if let Some(versions) = value.as_array_mut() {
            for version in versions {
                for key in ["id", "created_at"] {
                    if let Some(value) = version
                        .as_object_mut()
                        .and_then(|object| object.get_mut(key))
                    {
                        *value = Value::String(format!("<{key}>"));
                    }
                }
            }
        }
        value
    }

    async fn create_note_comment(server: &axum_test::TestServer) -> Value {
        let note_id = create_note(server, "body").await;
        let version_id = first_note_version_id(server, &note_id, "note", "body").await;
        let response = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "note_version",
                "target_id": version_id,
                "body": "fix this",
            }))
            .await;
        let body = response.json::<Value>();
        assert_eq!(
            (response.status_code(), normalize(body.clone())),
            (
                StatusCode::CREATED,
                expected_comment("note_version", None, "fix this", None, None, None, None),
            ),
        );
        body
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
        assert_eq!(
            (reply.status_code(), normalize(reply.json::<Value>())),
            (
                StatusCode::CREATED,
                expected_comment(
                    "note_version",
                    Some(root["id"].as_str().expect("root id")),
                    "sample reply",
                    None,
                    None,
                    None,
                    None,
                ),
            ),
        );
        let list = server
            .get(&format!(
                "/api/comments?target_kind=note_version&target_id={}",
                root["target_id"].as_str().expect("target id"),
            ))
            .await;
        assert_eq!(
            (list.status_code(), normalize_comment_list(list.json())),
            (
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
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::OK,
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
            ),
        );

        let res = server
            .patch(&format!("/api/comments/{id}"))
            .json(&json!({ "resolved": false }))
            .await;
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::OK,
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
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn update_comment_missing_id_is_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = uuid::Uuid::new_v4();
        let res = server
            .patch(&format!("/api/comments/{id}"))
            .json(&json!({ "resolved": true }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("comment {id} not found") })),
        );
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
        let reply = reply_response.json::<Value>();
        assert_eq!(
            (reply_response.status_code(), normalize(reply.clone())),
            (
                StatusCode::CREATED,
                expected_comment(
                    "note_version",
                    Some(root["id"].as_str().expect("root id")),
                    "first reply",
                    None,
                    None,
                    None,
                    None,
                ),
            ),
        );

        let nested_response = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": root["target_kind"],
                "target_id": root["target_id"],
                "parent_id": reply["id"],
                "body": "nested reply",
            }))
            .await;
        assert_response_eq(
            &nested_response,
            StatusCode::BAD_REQUEST,
            Some(json!({
                "error": "cannot reply to a reply; parent_id must reference a top-level comment",
            })),
        );
    }

    async fn create_note(server: &axum_test::TestServer, body_md: &str) -> String {
        let created = server
            .post("/api/notes")
            .json(&json!({
                "title": "note",
                "body_md": body_md,
            }))
            .await;
        let body = created.json::<Value>();
        assert_eq!(
            (created.status_code(), normalize_note_response(body.clone())),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "version_id": "<version_id>",
                    "version_no": 1,
                    "is_current": true,
                    "title": "note",
                    "body_md": body_md,
                    "frontmatter_json": {},
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
            ),
        );
        body["id"].as_str().expect("id").to_string()
    }

    async fn first_note_version_id(
        server: &axum_test::TestServer,
        note_id: &str,
        title: &str,
        body_md: &str,
    ) -> String {
        let versions = server.get(&format!("/api/notes/{note_id}/versions")).await;
        let body = versions.json::<Value>();
        assert_eq!(
            (
                versions.status_code(),
                normalize_note_version_list(body.clone())
            ),
            (
                StatusCode::OK,
                json!([{
                    "id": "<id>",
                    "note_id": note_id,
                    "version_no": 1,
                    "title": title,
                    "body_md": body_md,
                    "frontmatter_json": {},
                    "graphs_json": [],
                    "status": "approved",
                    "is_current": true,
                    "change_reason": null,
                    "created_by_kind": "human",
                    "execution_id": null,
                    "created_at": "<created_at>",
                    "reviewed_at": null,
                }]),
            ),
        );
        body[0]["id"].as_str().expect("version id").to_string()
    }

    async fn create_annotation(server: &axum_test::TestServer) -> String {
        let created = server
            .post("/api/annotations")
            .json(&json!({
                "target_symbol": "demo-code",
                "target_kind": "sample-kind",
                "timestamp": "2026-01-01T00:00:00Z",
                "text": "text",
            }))
            .await;
        let body = created.json::<Value>();
        assert_eq!(
            (
                created.status_code(),
                normalize_annotation_response(body.clone())
            ),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "target_symbol": "demo-code",
                    "target_kind": "sample-kind",
                    "timestamp": "2026-01-01T00:00:00Z",
                    "price": null,
                    "text": "text",
                    "status": "unread",
                    "linked_note_id": null,
                    "created_by_kind": "human",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                    "execution_step_id": null,
                    "execution_task_id": null,
                }),
            ),
        );
        body["id"].as_str().expect("id").to_string()
    }

    #[backend_test_macros::database_test]
    async fn create_comment_with_line_anchor_stores_explicit_lines(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let note_id = create_note(
            &server,
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;
        let version_id = first_note_version_id(
            &server,
            &note_id,
            "note",
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;

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
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::CREATED,
                expected_comment(
                    "note_version",
                    None,
                    "fix this line",
                    Some("line two"),
                    Some("new"),
                    Some(2),
                    Some(2),
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_comment_keeps_quote_with_explicit_line_anchor(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let note_id = create_note(
            &server,
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;
        let version_id = first_note_version_id(
            &server,
            &note_id,
            "note",
            indoc::indoc! {"
                line one
                line two
                line three"},
        )
        .await;

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
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::CREATED,
                expected_comment(
                    "note_version",
                    None,
                    "fix this line",
                    Some("line that no longer exists"),
                    Some("new"),
                    Some(3),
                    Some(3),
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_comment_on_annotation_with_anchor_text_saves_text_without_line_numbers(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let server = create_test_server(db).await;
        let annotation_id = create_annotation(&server).await;

        let res = server
            .post("/api/comments")
            .json(&json!({
                "target_kind": "annotation",
                "target_id": annotation_id,
                "body": "fix this",
                "anchor_text": "some selected text",
            }))
            .await;
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::CREATED,
                expected_comment(
                    "annotation",
                    None,
                    "fix this",
                    Some("some selected text"),
                    None,
                    None,
                    None,
                ),
            ),
        );
    }
}
