#[cfg(test)]
mod tests {
    use super::super::assert_response_eq;
    use crate::testing::create_test_server;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use serde_json::{Value, json};

    async fn create_feed(
        server: &TestServer,
        source: &str,
        display_name: &str,
        url: &str,
    ) -> String {
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": source,
                "display_name": display_name,
                "url": url,
            }))
            .await;
        let created: Value = res.json();
        assert_eq!(
            (res.status_code(), normalize(created.clone())),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "source": source,
                    "display_name": display_name,
                    "url": url,
                    "enabled": true,
                    "content_source": "none",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        created["id"].as_str().unwrap().to_owned()
    }

    fn normalize(mut value: Value) -> Value {
        for key in ["id", "created_at", "updated_at"] {
            if let Some(v) = value.get_mut(key) {
                *v = Value::String(format!("<{key}>"));
            }
        }
        value
    }

    #[backend_test_macros::database_test]
    async fn create_returns_201_with_full_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "sample-newswire",
                "display_name": "Sample Newswire",
                "url": "https://feeds.example.invalid/markets.xml",
            }))
            .await;
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "source": "sample-newswire",
                    "display_name": "Sample Newswire",
                    "url": "https://feeds.example.invalid/markets.xml",
                    "enabled": true,
                    "content_source": "none",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_accepts_content_source(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "sample-feed",
                "display_name": "Sample Feed",
                "url": "https://feeds.example.invalid/sample.xml",
                "content_source": "feed",
            }))
            .await;
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "source": "sample-feed",
                    "display_name": "Sample Feed",
                    "url": "https://feeds.example.invalid/sample.xml",
                    "enabled": true,
                    "content_source": "feed",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_returns_full_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_feed(
            &server,
            "example-feed",
            "Example Feed",
            "https://example.com/feed.xml",
        )
        .await;

        let res = server.get(&format!("/api/rss-feeds/{id}")).await;
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "source": "example-feed",
                    "display_name": "Example Feed",
                    "url": "https://example.com/feed.xml",
                    "enabled": true,
                    "content_source": "none",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_feed_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let missing_id = "00000000-0000-4000-8000-000000000001";
        let res = server.get(&format!("/api/rss-feeds/{missing_id}")).await;
        assert_response_eq(
            &res,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("rss feed {missing_id} not found") })),
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rejects_unknown_field(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "example-feed",
                "display_name": "Example Feed",
                "url": "https://example.com/feed.xml",
                "unexpected": true,
            }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(json!({
                "error": "Failed to deserialize the JSON body into the target type: unexpected: unknown field `unexpected`, expected one of `source`, `display_name`, `url`, `enabled`, `content_source` at line 1 column 67"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn duplicate_source_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = json!({
            "source": "dup",
            "display_name": "Dup",
            "url": "https://example.com/a",
        });
        let created = server.post("/api/rss-feeds").json(&body).await;
        let created_body: Value = created.json();
        assert_eq!(
            (created.status_code(), normalize(created_body)),
            (
                StatusCode::CREATED,
                json!({
                    "id": "<id>",
                    "source": "dup",
                    "display_name": "Dup",
                    "url": "https://example.com/a",
                    "enabled": true,
                    "content_source": "none",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        let res = server.post("/api/rss-feeds").json(&body).await;
        assert_response_eq(
            &res,
            StatusCode::CONFLICT,
            Some(json!({ "error": "rss feed with source 'dup' already exists" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn invalid_source_slug_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "Has Space",
                "display_name": "x",
                "url": "https://example.com/a",
            }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({
                "error": "source must match ^[a-z0-9_-]+$ (got 'Has Space')"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn invalid_url_is_400(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/rss-feeds")
            .json(&json!({
                "source": "ok",
                "display_name": "x",
                "url": "not-a-url",
            }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::BAD_REQUEST,
            Some(json!({ "error": "url must be a valid http(s) URL (got 'not-a-url')" })),
        );
    }

    #[backend_test_macros::database_test]
    async fn list_enabled_only_filters(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let a_id = create_feed(&server, "a", "A", "https://example.com/a").await;
        create_feed(&server, "b", "B", "https://example.com/b").await;
        let disabled = server
            .patch(&format!("/api/rss-feeds/{a_id}"))
            .json(&json!({ "enabled": false }))
            .await;
        assert_eq!(
            (disabled.status_code(), normalize(disabled.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "source": "a",
                    "display_name": "A",
                    "url": "https://example.com/a",
                    "enabled": false,
                    "content_source": "none",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
        let res = server.get("/api/rss-feeds?enabled_only=true").await;
        let body: Vec<Value> = res.json();
        let normalized: Vec<Value> = body.into_iter().map(normalize).collect();
        assert_eq!(
            (res.status_code(), normalized),
            (
                StatusCode::OK,
                vec![json!({
                    "id": "<id>",
                    "source": "b",
                    "display_name": "B",
                    "url": "https://example.com/b",
                    "enabled": true,
                    "content_source": "none",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                })],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn patch_updates_fields(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_feed(&server, "x", "Old", "https://example.com/a").await;
        let res = server
            .patch(&format!("/api/rss-feeds/{id}"))
            .json(&json!({ "display_name": "New", "enabled": false, "content_source": "crawl" }))
            .await;
        assert_eq!(
            (res.status_code(), normalize(res.json())),
            (
                StatusCode::OK,
                json!({
                    "id": "<id>",
                    "source": "x",
                    "display_name": "New",
                    "url": "https://example.com/a",
                    "enabled": false,
                    "content_source": "crawl",
                    "created_at": "<created_at>",
                    "updated_at": "<updated_at>",
                }),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn patch_rejects_unknown_field(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_feed(
            &server,
            "example-feed",
            "Example Feed",
            "https://example.com/feed.xml",
        )
        .await;

        let res = server
            .patch(&format!("/api/rss-feeds/{id}"))
            .json(&json!({
                "display_name": "Renamed Feed",
                "unexpected": true,
            }))
            .await;
        assert_response_eq(
            &res,
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(json!({
                "error": "Failed to deserialize the JSON body into the target type: unexpected: unknown field `unexpected`, expected one of `display_name`, `url`, `enabled`, `content_source` at line 1 column 43"
            })),
        );
    }

    #[backend_test_macros::database_test]
    async fn delete_returns_204_then_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_feed(&server, "x", "x", "https://example.com/a").await;
        let deleted = server.delete(&format!("/api/rss-feeds/{id}")).await;
        assert_response_eq(&deleted, StatusCode::NO_CONTENT, None);
        let missing = server.delete(&format!("/api/rss-feeds/{id}")).await;
        assert_response_eq(
            &missing,
            StatusCode::NOT_FOUND,
            Some(json!({ "error": format!("rss feed {id} not found") })),
        );
    }
}
