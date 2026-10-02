//! 管理 MCP の RSS フィード一覧 tool。

use core_application::rss_feed::{RssFeedRepositoryError, RssFeedUseCaseError};
use rmcp::ErrorData as McpError;

use super::MgmtServer;
use super::dto::{ListRssFeedsParams, ListRssFeedsResult};
use super::invalid_params;

impl MgmtServer {
    pub(super) async fn list_rss_feeds_inner(
        &self,
        params: ListRssFeedsParams,
    ) -> Result<ListRssFeedsResult, McpError> {
        let rows = self
            .use_cases
            .rss_feeds()
            .list(params.enabled_only.unwrap_or(false))
            .await
            .map_err(map_rss_feed_error)?;
        Ok(ListRssFeedsResult {
            feeds: rows.into_iter().map(Into::into).collect(),
        })
    }
}

fn map_rss_feed_error(err: RssFeedUseCaseError) -> McpError {
    match err {
        RssFeedUseCaseError::Validation(message) => invalid_params(message),
        error @ RssFeedUseCaseError::Repository(RssFeedRepositoryError::DuplicateSource(_)) => {
            invalid_params(error.to_string())
        }
        error @ RssFeedUseCaseError::NotFound(_) => {
            McpError::resource_not_found(error.to_string(), None)
        }
        RssFeedUseCaseError::Repository(error) => {
            tracing::error!(error = %error, "mgmt mcp rss feed operation failed");
            super::internal_error(format!("database error: {error}"))
        }
        RssFeedUseCaseError::UnitOfWork(error) => {
            tracing::error!(error = %error, "mgmt mcp rss feed transaction failed");
            super::internal_error(format!("database error: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use uuid::Uuid;

    use super::super::tests_common::build_server;
    use super::*;
    use crate::agent_client::FakeAgentTaskClient;
    use core_application::rss_feed::CreateRssFeedCommand;

    #[backend_test_macros::database_test]
    async fn list_rss_feeds_returns_configured_feeds(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));
        server
            .use_cases
            .rss_feeds()
            .create(CreateRssFeedCommand {
                source: "sample-newswire".into(),
                display_name: "Sample Newswire".into(),
                url: "https://feeds.example.invalid/markets.xml".into(),
                enabled: None,
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
            })],
        );
    }
}
