use super::dto::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use uuid::Uuid;

    use super::super::tests_common::build_server;
    use super::*;
    use core_application::agent_task_client::FakeAgentTaskClient;
    use core_application::rss_feed::CreateRssFeedCommand;

    #[backend_test_macros::database_test]
    async fn list_rss_feeds_returns_configured_feeds(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));
        server
            .dependencies
            .rss_feeds
            .create(CreateRssFeedCommand {
                source: "sample-newswire".into(),
                display_name: "Sample Newswire".into(),
                url: "https://feeds.example.invalid/markets.xml".into(),
                enabled: None,
                content_source: "none".into(),
            })
            .await
            .expect("seed RSS feed");

        let Json(listed) = server
            .list_rss_feeds(Parameters(ListRssFeedsParams { enabled_only: None }))
            .await
            .expect("list ok");

        let feeds = listed
            .feeds
            .into_iter()
            .map(|feed| {
                serde_json::to_value(super::super::dto::RssFeedSummary {
                    id: Uuid::nil(),
                    ..feed
                })
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            feeds,
            vec![serde_json::json!({
                "id": Uuid::nil(),
                "source": "sample-newswire",
                "display_name": "Sample Newswire",
                "url": "https://feeds.example.invalid/markets.xml",
                "enabled": true,
                "content_source": "none",
            })],
        );
    }

    #[backend_test_macros::database_test]
    async fn update_rss_feed_changes_content_source(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));
        let created = server
            .dependencies
            .rss_feeds
            .create(CreateRssFeedCommand {
                source: "fictional-feed".into(),
                display_name: "Fictional Feed".into(),
                url: "https://feeds.example.invalid/fictional.xml".into(),
                enabled: None,
                content_source: "none".into(),
            })
            .await
            .expect("seed RSS feed");

        server
            .update_rss_feed(Parameters(UpdateRssFeedParams {
                id: created.id,
                content_source: Some("crawl".into()),
            }))
            .await
            .expect("update content source");

        let Json(listed) = server
            .list_rss_feeds(Parameters(ListRssFeedsParams { enabled_only: None }))
            .await
            .expect("list ok");

        let feeds = listed
            .feeds
            .into_iter()
            .map(|feed| {
                serde_json::to_value(super::super::dto::RssFeedSummary {
                    id: Uuid::nil(),
                    ..feed
                })
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            feeds,
            vec![serde_json::json!({
                "id": Uuid::nil(),
                "source": "fictional-feed",
                "display_name": "Fictional Feed",
                "url": "https://feeds.example.invalid/fictional.xml",
                "enabled": true,
                "content_source": "crawl",
            })],
        );
    }
}
