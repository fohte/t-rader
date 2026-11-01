#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use rstest::{fixture, rstest};
    use serde_json::{Value, json};

    use crate::testing::create_test_server;

    struct NoteTagTestContext {
        server: axum_test::TestServer,
    }

    #[fixture]
    async fn note_tag_test_context() -> NoteTagTestContext {
        let db = gateway_postgres::test_support::create_test_transaction().await;
        let server = create_test_server(db).await;
        NoteTagTestContext { server }
    }

    #[rstest]
    #[tokio::test]
    async fn create_preserves_an_arbitrary_key(
        #[future] note_tag_test_context: NoteTagTestContext,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let response = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "Mixed/テーマ",
                "display_name": "Sample label",
                "description": "A synthetic label",
            }))
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (
                StatusCode::CREATED,
                json!({
                    "key": "Mixed/テーマ",
                    "display_name": "Sample label",
                    "description": "A synthetic label",
                }),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn list_sorts_tags_by_display_name(#[future] note_tag_test_context: NoteTagTestContext) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let first = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key-z",
                "display_name": "Zeta",
            }))
            .await;
        let second = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key-a",
                "display_name": "Alpha",
            }))
            .await;
        let listed = server.get("/api/note-tags").await;

        assert_eq!(
            (
                first.status_code(),
                second.status_code(),
                listed.status_code(),
                listed.json::<Value>(),
            ),
            (
                StatusCode::CREATED,
                StatusCode::CREATED,
                StatusCode::OK,
                json!([
                    {
                        "key": "sample-key-a",
                        "display_name": "Alpha",
                        "description": null,
                    },
                    {
                        "key": "sample-key-z",
                        "display_name": "Zeta",
                        "description": null,
                    },
                ]),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn list_sorts_tags_by_key_when_display_names_match(
        #[future] note_tag_test_context: NoteTagTestContext,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let second_by_key = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key-z",
                "display_name": "Shared label",
            }))
            .await;
        let first_by_key = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key-a",
                "display_name": "Shared label",
            }))
            .await;
        let listed = server.get("/api/note-tags").await;

        assert_eq!(
            (
                second_by_key.status_code(),
                first_by_key.status_code(),
                listed.status_code(),
                listed.json::<Value>(),
            ),
            (
                StatusCode::CREATED,
                StatusCode::CREATED,
                StatusCode::OK,
                json!([
                    {
                        "key": "sample-key-a",
                        "display_name": "Shared label",
                        "description": null,
                    },
                    {
                        "key": "sample-key-z",
                        "display_name": "Shared label",
                        "description": null,
                    },
                ]),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_changes_display_name_and_clears_description(
        #[future] note_tag_test_context: NoteTagTestContext,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let created = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "Mixed/テーマ",
                "display_name": "Sample label",
                "description": "A synthetic label",
            }))
            .await;
        let updated = server
            .patch("/api/note-tags/Mixed%2F%E3%83%86%E3%83%BC%E3%83%9E")
            .json(&json!({
                "display_name": "Updated label",
                "description": null,
            }))
            .await;

        assert_eq!(
            (
                created.status_code(),
                updated.status_code(),
                updated.json::<Value>()
            ),
            (
                StatusCode::CREATED,
                StatusCode::OK,
                json!({
                    "key": "Mixed/テーマ",
                    "display_name": "Updated label",
                    "description": null,
                }),
            ),
        );
    }

    #[rstest]
    #[case::missing_tag(
        "missing-key",
        "Updated label",
        StatusCode::NOT_FOUND,
        "note tag missing-key not found"
    )]
    #[case::blank_display_name(
        "sample-key",
        " \t ",
        StatusCode::BAD_REQUEST,
        "display_name must not be empty"
    )]
    #[tokio::test]
    async fn update_rejects_unknown_tag_and_blank_display_name(
        #[future] note_tag_test_context: NoteTagTestContext,
        #[case] key: &str,
        #[case] display_name: &str,
        #[case] expected_status: StatusCode,
        #[case] expected_error: &str,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let created = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key",
                "display_name": "Sample label",
            }))
            .await;
        let response = server
            .patch(&format!("/api/note-tags/{key}"))
            .json(&json!({ "display_name": display_name }))
            .await;

        assert_eq!(
            (
                created.status_code(),
                response.status_code(),
                response.json::<Value>()
            ),
            (
                StatusCode::CREATED,
                expected_status,
                json!({ "error": expected_error }),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn update_without_fields_preserves_the_tag(
        #[future] note_tag_test_context: NoteTagTestContext,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let created = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key",
                "display_name": "Sample label",
                "description": "A synthetic label",
            }))
            .await;
        let updated = server
            .patch("/api/note-tags/sample-key")
            .json(&json!({}))
            .await;

        assert_eq!(
            (
                created.status_code(),
                updated.status_code(),
                updated.json::<Value>()
            ),
            (
                StatusCode::CREATED,
                StatusCode::OK,
                json!({
                    "key": "sample-key",
                    "display_name": "Sample label",
                    "description": "A synthetic label",
                }),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn create_rejects_duplicate_keys(#[future] note_tag_test_context: NoteTagTestContext) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let first = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key",
                "display_name": "Sample label",
            }))
            .await;
        let duplicate = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key",
                "display_name": "Another label",
            }))
            .await;

        assert_eq!(
            (
                first.status_code(),
                duplicate.status_code(),
                duplicate.json::<Value>()
            ),
            (
                StatusCode::CREATED,
                StatusCode::CONFLICT,
                json!({ "error": "note tag sample-key already exists" }),
            ),
        );
    }

    #[rstest]
    #[case::blank_key("   ", "Sample label", "key must not be empty")]
    #[case::blank_display_name("sample-key", " \t ", "display_name must not be empty")]
    #[tokio::test]
    async fn create_rejects_blank_key_and_display_name(
        #[future] note_tag_test_context: NoteTagTestContext,
        #[case] key: &str,
        #[case] display_name: &str,
        #[case] expected_error: &str,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let response = server
            .post("/api/note-tags")
            .json(&json!({ "key": key, "display_name": display_name }))
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (StatusCode::BAD_REQUEST, json!({ "error": expected_error }),),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn delete_removes_the_tag(#[future] note_tag_test_context: NoteTagTestContext) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let created = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample-key",
                "display_name": "Sample label",
            }))
            .await;
        let deleted = server.delete("/api/note-tags/sample-key").await;
        let listed = server.get("/api/note-tags").await;

        assert_eq!(
            (
                created.status_code(),
                deleted.status_code(),
                listed.status_code(),
                listed.json::<Value>(),
            ),
            (
                StatusCode::CREATED,
                StatusCode::NO_CONTENT,
                StatusCode::OK,
                json!([]),
            ),
        );
    }
}
