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
    async fn note_tag_api_manages_unrestricted_keys_and_lists_by_display_name(
        #[future] note_tag_test_context: NoteTagTestContext,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let first = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "Mixed/テーマ",
                "display_name": "Zeta",
                "description": "A synthetic label",
            }))
            .await;
        let second = server
            .post("/api/note-tags")
            .json(&json!({
                "key": "sample.tag+one",
                "display_name": "Alpha",
            }))
            .await;
        let updated = server
            .patch("/api/note-tags/Mixed%2F%E3%83%86%E3%83%BC%E3%83%9E")
            .json(&json!({
                "display_name": "Beta",
                "description": null,
            }))
            .await;
        let listed = server.get("/api/note-tags").await;
        let deleted = server
            .delete("/api/note-tags/Mixed%2F%E3%83%86%E3%83%BC%E3%83%9E")
            .await;
        let repeated_delete = server
            .delete("/api/note-tags/Mixed%2F%E3%83%86%E3%83%BC%E3%83%9E")
            .await;

        assert_eq!(
            (
                first.status_code(),
                first.json::<Value>(),
                second.status_code(),
                second.json::<Value>(),
                updated.status_code(),
                updated.json::<Value>(),
                listed.status_code(),
                listed.json::<Value>(),
                deleted.status_code(),
                repeated_delete.status_code(),
            ),
            (
                StatusCode::CREATED,
                json!({
                    "key": "Mixed/テーマ",
                    "display_name": "Zeta",
                    "description": "A synthetic label",
                }),
                StatusCode::CREATED,
                json!({
                    "key": "sample.tag+one",
                    "display_name": "Alpha",
                    "description": null,
                }),
                StatusCode::OK,
                json!({
                    "key": "Mixed/テーマ",
                    "display_name": "Beta",
                    "description": null,
                }),
                StatusCode::OK,
                json!([
                    {
                        "key": "sample.tag+one",
                        "display_name": "Alpha",
                        "description": null,
                    },
                    {
                        "key": "Mixed/テーマ",
                        "display_name": "Beta",
                        "description": null,
                    },
                ]),
                StatusCode::NO_CONTENT,
                StatusCode::NOT_FOUND,
            ),
        );
    }

    #[rstest]
    #[case::blank_key("   ", "label", "key must not be empty")]
    #[case::blank_display_name("sample-key", " \t ", "display_name must not be empty")]
    #[tokio::test]
    async fn note_tag_api_rejects_blank_key_and_display_name(
        #[future] note_tag_test_context: NoteTagTestContext,
        #[case] key: &str,
        #[case] display_name: &str,
        #[case] message: &str,
    ) {
        let NoteTagTestContext { server } = note_tag_test_context.await;
        let response = server
            .post("/api/note-tags")
            .json(&json!({ "key": key, "display_name": display_name }))
            .await;

        assert_eq!(
            (response.status_code(), response.json::<Value>()),
            (StatusCode::BAD_REQUEST, json!({ "error": message }),),
        );
    }
}
