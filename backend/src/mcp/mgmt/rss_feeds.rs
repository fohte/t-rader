//! 管理 MCP の RSS フィード CRUD tool。

use rmcp::ErrorData as McpError;

use crate::services::rss_feed as rss_feed_svc;

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
        let rows = rss_feed_svc::list(&self.db, params.enabled_only.unwrap_or(false))
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
        let created = rss_feed_svc::create(
            &self.db,
            rss_feed_svc::CreateInput {
                source: params.source,
                display_name: params.display_name,
                url: params.url,
                enabled: params.enabled,
            },
        )
        .await
        .map_err(map_rss_feed_error)?;
        Ok(created.into())
    }

    pub(super) async fn update_rss_feed_inner(
        &self,
        params: UpdateRssFeedParams,
    ) -> Result<RssFeedSummary, McpError> {
        let updated = rss_feed_svc::update(
            &self.db,
            params.id,
            rss_feed_svc::UpdatePatch {
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
        rss_feed_svc::delete(&self.db, params.id)
            .await
            .map_err(map_rss_feed_error)?;
        Ok(DeleteRssFeedResult { id: params.id })
    }
}

fn map_rss_feed_error(err: rss_feed_svc::RssFeedError) -> McpError {
    match err {
        rss_feed_svc::RssFeedError::InvalidSource(_)
        | rss_feed_svc::RssFeedError::InvalidUrl(_)
        | rss_feed_svc::RssFeedError::EmptyDisplayName => invalid_params(err.to_string()),
        rss_feed_svc::RssFeedError::DuplicateSource(_) => invalid_params(err.to_string()),
        rss_feed_svc::RssFeedError::NotFound(_) => {
            McpError::resource_not_found(err.to_string(), None)
        }
        rss_feed_svc::RssFeedError::Database(e) => super::db_error(e),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::create_test_db;

    use super::super::tests_common::build_server;
    use super::*;

    #[sqlx::test(migrations = false)]
    async fn create_rss_feed_inserts_and_lists(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db.clone(), Arc::new(FakeAgentTaskClient::new()));

        let Json(created) = server
            .create_rss_feed(Parameters(CreateRssFeedParams {
                source: "bloomberg-jp".into(),
                display_name: "Bloomberg JP".into(),
                url: "https://feeds.bloomberg.co.jp/markets.xml".into(),
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
            source: "bloomberg-jp".into(),
            display_name: "Bloomberg JP".into(),
            url: "https://feeds.bloomberg.co.jp/markets.xml".into(),
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

    #[sqlx::test(migrations = false)]
    async fn create_rss_feed_rejects_invalid_source(pool: PgPool) {
        let db = create_test_db(pool).await;
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
