//! 管理 MCP の RSS フィード CRUD tool。

use core_application::rss_feed::{
    CreateRssFeedCommand, RssFeedRepositoryError, RssFeedUseCaseError, UpdateRssFeedPatch,
};
use rmcp::ErrorData as McpError;

use super::MgmtServer;
use super::dto::{
    CreateRssFeedParams, DeleteRssFeedParams, DeleteRssFeedResult, ListRssFeedsParams,
    ListRssFeedsResult, RssFeedSummary, UpdateRssFeedParams,
};
use super::invalid_params;

impl MgmtServer {
    pub(super) async fn list_rss_feeds_inner(
        &self,
        params: ListRssFeedsParams,
    ) -> Result<ListRssFeedsResult, McpError> {
        let rows = self
            .use_cases
            .rss_feeds
            .list(params.enabled_only.unwrap_or(false))
            .await
            .map_err(map_rss_feed_error)?;
        Ok(ListRssFeedsResult {
            feeds: rows.into_iter().map(Into::into).collect(),
        })
    }

    pub(super) async fn create_rss_feed_inner(
        &self,
        params: CreateRssFeedParams,
    ) -> Result<RssFeedSummary, McpError> {
        let created = self
            .use_cases
            .rss_feeds
            .create(CreateRssFeedCommand {
                source: params.source,
                display_name: params.display_name,
                url: params.url,
                enabled: params.enabled,
            })
            .await
            .map_err(map_rss_feed_error)?;
        Ok(created.into())
    }

    pub(super) async fn update_rss_feed_inner(
        &self,
        params: UpdateRssFeedParams,
    ) -> Result<RssFeedSummary, McpError> {
        let updated = self
            .use_cases
            .rss_feeds
            .update(
                params.id,
                UpdateRssFeedPatch {
                    display_name: params.display_name,
                    url: params.url,
                    enabled: params.enabled,
                },
            )
            .await
            .map_err(map_rss_feed_error)?;
        Ok(updated.into())
    }

    pub(super) async fn delete_rss_feed_inner(
        &self,
        params: DeleteRssFeedParams,
    ) -> Result<DeleteRssFeedResult, McpError> {
        self.use_cases
            .rss_feeds
            .delete(params.id)
            .await
            .map_err(map_rss_feed_error)?;
        Ok(DeleteRssFeedResult { id: params.id })
    }
}

fn map_rss_feed_error(err: RssFeedUseCaseError) -> McpError {
    match err {
        RssFeedUseCaseError::Validation(message) => invalid_params(message),
        error @ RssFeedUseCaseError::Repository(RssFeedRepositoryError::DuplicateSource(_)) => {
            invalid_params(error.to_string())
        }
        RssFeedUseCaseError::NotFound(id) => {
            McpError::resource_not_found(RssFeedUseCaseError::NotFound(id).to_string(), None)
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

    #[backend_test_macros::database_test]
    async fn create_rss_feed_inserts_and_lists(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(created) = server
            .create_rss_feed(Parameters(CreateRssFeedParams {
                source: "sample-newswire".into(),
                display_name: "Sample Newswire".into(),
                url: "https://feeds.example.invalid/markets.xml".into(),
                enabled: None,
            }))
            .await
            .expect("create ok");
        let summary = |s: RssFeedSummary| RssFeedSummary {
            id: Uuid::nil(),
            ..s
        };
        let expected = RssFeedSummary {
            id: Uuid::nil(),
            source: "sample-newswire".into(),
            display_name: "Sample Newswire".into(),
            url: "https://feeds.example.invalid/markets.xml".into(),
            enabled: true,
        };
        assert_eq!(
            serde_json::to_value(summary(created)).unwrap(),
            serde_json::to_value(&expected).unwrap(),
        );

        let Json(listed) = server
            .list_rss_feeds(Parameters(ListRssFeedsParams { enabled_only: None }))
            .await
            .expect("list ok");
        assert_eq!(
            listed
                .feeds
                .into_iter()
                .map(summary)
                .map(|s| serde_json::to_value(s).unwrap())
                .collect::<Vec<_>>(),
            vec![serde_json::to_value(&expected).unwrap()],
        );
    }

    #[backend_test_macros::database_test]
    async fn create_rss_feed_rejects_invalid_source(db: gateway_postgres::DatabaseHandle) {
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let err = server
            .create_rss_feed(Parameters(CreateRssFeedParams {
                source: "Bad Source".into(),
                display_name: "x".into(),
                url: "https://example.com/a".into(),
                enabled: None,
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
