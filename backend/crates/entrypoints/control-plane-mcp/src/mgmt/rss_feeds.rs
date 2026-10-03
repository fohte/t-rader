//! 管理 MCP の RSS フィード一覧・設定更新 tool。

use core_application::rss_feed::{RssFeedRepositoryError, RssFeedUseCaseError, UpdateRssFeedPatch};
use rmcp::ErrorData as McpError;

use super::MgmtServer;
use super::dto::{ListRssFeedsParams, ListRssFeedsResult, RssFeedSummary, UpdateRssFeedParams};
use super::invalid_params;

impl MgmtServer {
    pub(super) async fn list_rss_feeds_inner(
        &self,
        params: ListRssFeedsParams,
    ) -> Result<ListRssFeedsResult, McpError> {
        let rows = self
            .dependencies
            .rss_feeds
            .list(params.enabled_only.unwrap_or(false))
            .await
            .map_err(map_rss_feed_error)?;
        Ok(ListRssFeedsResult {
            feeds: rows.into_iter().map(Into::into).collect(),
        })
    }

    pub(super) async fn update_rss_feed_inner(
        &self,
        params: UpdateRssFeedParams,
    ) -> Result<RssFeedSummary, McpError> {
        self.dependencies
            .rss_feeds
            .update(
                params.id,
                UpdateRssFeedPatch {
                    content_source: params.content_source,
                    ..Default::default()
                },
            )
            .await
            .map(Into::into)
            .map_err(map_rss_feed_error)
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
