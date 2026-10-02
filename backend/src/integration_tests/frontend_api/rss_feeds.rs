#[cfg(test)]
mod tests {
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
        res.assert_status(StatusCode::CREATED);
        let created: Value = res.json();
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
        res.assert_status(StatusCode::CREATED);
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "source": "sample-newswire",
                "display_name": "Sample Newswire",
                "url": "https://feeds.example.invalid/markets.xml",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
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
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "source": "example-feed",
                "display_name": "Example Feed",
                "url": "https://example.com/feed.xml",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_feed_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/rss-feeds/00000000-0000-4000-8000-000000000001")
            .await;
        res.assert_status(StatusCode::NOT_FOUND);
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
        res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[backend_test_macros::database_test]
    async fn duplicate_source_is_409(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let body = json!({
            "source": "dup",
            "display_name": "Dup",
            "url": "https://example.com/a",
        });
        server
            .post("/api/rss-feeds")
            .json(&body)
            .await
            .assert_status(StatusCode::CREATED);
        let res = server.post("/api/rss-feeds").json(&body).await;
        res.assert_status(StatusCode::CONFLICT);
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
        res.assert_status(StatusCode::BAD_REQUEST);
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
        res.assert_status(StatusCode::BAD_REQUEST);
    }

    #[backend_test_macros::database_test]
    async fn list_enabled_only_filters(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let a_id = create_feed(&server, "a", "A", "https://example.com/a").await;
        create_feed(&server, "b", "B", "https://example.com/b").await;
        server
            .patch(&format!("/api/rss-feeds/{a_id}"))
            .json(&json!({ "enabled": false }))
            .await
            .assert_status_ok();
        let res = server.get("/api/rss-feeds?enabled_only=true").await;
        res.assert_status_ok();
        let body: Vec<Value> = res.json();
        let normalized: Vec<Value> = body.into_iter().map(normalize).collect();
        assert_eq!(
            normalized,
            vec![json!({
                "id": "<id>",
                "source": "b",
                "display_name": "B",
                "url": "https://example.com/b",
                "enabled": true,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            })],
        );
    }

    #[backend_test_macros::database_test]
    async fn patch_updates_fields(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_feed(&server, "x", "Old", "https://example.com/a").await;
        let res = server
            .patch(&format!("/api/rss-feeds/{id}"))
            .json(&json!({ "display_name": "New", "enabled": false }))
            .await;
        res.assert_status_ok();
        assert_eq!(
            normalize(res.json()),
            json!({
                "id": "<id>",
                "source": "x",
                "display_name": "New",
                "url": "https://example.com/a",
                "enabled": false,
                "created_at": "<created_at>",
                "updated_at": "<updated_at>",
            }),
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
        res.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[backend_test_macros::database_test]
    async fn delete_returns_204_then_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_feed(&server, "x", "x", "https://example.com/a").await;
        server
            .delete(&format!("/api/rss-feeds/{id}"))
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .delete(&format!("/api/rss-feeds/{id}"))
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }
}
