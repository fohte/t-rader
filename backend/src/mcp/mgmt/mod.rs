//! 管理 MCP server の tool 実装
//!
//! personal-bot から呼び出される。tool は以下の 11 種:
//!
//! - `list_strategies`
//! - `submit_strategy_task`
//! - `get_strategy_task_status`
//! - `get_strategy_agent_config`
//! - `put_strategy_agent_config`
//! - `list_recent_notes`
//! - `list_recent_annotations`
//! - `list_rss_feeds`
//! - `create_rss_feed`
//! - `update_rss_feed`
//! - `delete_rss_feed`
//!
//! 実装はドメインごとに分割している:
//!
//! - `dto`: 各 tool の入出力スキーマ
//! - `strategies`: 戦略一覧・タスク投入・タスク status
//!   (`list_strategies_inner` / `submit_strategy_task_inner` / `get_strategy_task_status_inner`)
//! - `agent_graph`: 戦略の多段フェーズ実行設定 (agent_graph YAML) 取得・保存
//!   (`get_strategy_agent_config_inner` / `put_strategy_agent_config_inner`)
//! - `rss_feeds`: RSS フィード CRUD
//!   (`list_rss_feeds_inner` / `create_rss_feed_inner` / `update_rss_feed_inner` / `delete_rss_feed_inner`)
//! - `notes_annotations`: 直近ノート・アノテーション一覧
//!   (`list_recent_notes_inner` / `list_recent_annotations_inner`)
//!
//! 本モジュールは tool wrapper (`#[tool_router]` / `#[tool_handler]`) と
//! 共通のエラー変換ヘルパを担う。

mod agent_graph;
pub(super) mod dto;
mod notes_annotations;
mod rss_feeds;
mod strategies;

#[cfg(test)]
mod tests_common;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use sea_orm::DatabaseConnection;

use crate::agent_client::SharedAgentTaskClient;

// `SubmitStrategyTaskParams` は integration_tests.rs からも直接参照されるため公開する。
pub use dto::SubmitStrategyTaskParams;
use dto::{
    CreateRssFeedParams, DeleteRssFeedParams, DeleteRssFeedResult, GetStrategyAgentConfigParams,
    GetStrategyAgentConfigResult, GetStrategyTaskStatusParams, GetStrategyTaskStatusResult,
    ListRecentAnnotationsResult, ListRecentNotesResult, ListRecentParams, ListRssFeedsParams,
    ListRssFeedsResult, ListStrategiesResult, PutStrategyAgentConfigParams,
    PutStrategyAgentConfigResult, RssFeedSummary, SubmitStrategyTaskResult, UpdateRssFeedParams,
};

const DEFAULT_LIST_LIMIT: u64 = 20;
const MAX_LIST_LIMIT: u64 = 100;

#[derive(Clone)]
pub struct MgmtServer {
    db: DatabaseConnection,
    agent_client: SharedAgentTaskClient,
}

impl MgmtServer {
    pub fn new(db: DatabaseConnection, agent_client: SharedAgentTaskClient) -> Self {
        Self { db, agent_client }
    }
}

// === MCP error マッピング ===

pub(super) fn internal_error(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::internal_error(msg, None)
}

pub(super) fn invalid_params(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::invalid_params(msg, None)
}

pub(super) fn db_error(err: sea_orm::DbErr) -> McpError {
    tracing::error!(error = %err, "mgmt mcp db error");
    internal_error(format!("database error: {err}"))
}

pub(super) fn clamp_limit(limit: Option<u32>) -> u64 {
    let value = limit.map(u64::from).unwrap_or(DEFAULT_LIST_LIMIT);
    value.clamp(1, MAX_LIST_LIMIT)
}

#[tool_router]
impl MgmtServer {
    /// 戦略一覧 (id / 名前 / 最終更新 / 未読カード数)
    #[tool(
        name = "list_strategies",
        description = "List all strategies with id, name, last updated time, and unread card count."
    )]
    async fn list_strategies(&self) -> Result<Json<ListStrategiesResult>, McpError> {
        self.list_strategies_inner().await.map(Json)
    }

    /// 戦略 id + prompt から t-rader-agent にタスクを投入し task_id / a2a_task_id を返す
    #[tool(
        name = "submit_strategy_task",
        description = "Submit a strategy task to t-rader-agent for the given strategy_id and prompt; returns task_id and a2a_task_id."
    )]
    pub(crate) async fn submit_strategy_task(
        &self,
        Parameters(params): Parameters<SubmitStrategyTaskParams>,
    ) -> Result<Json<SubmitStrategyTaskResult>, McpError> {
        self.submit_strategy_task_inner(params).await.map(Json)
    }

    /// 投入済み戦略タスクの status を返す
    #[tool(
        name = "get_strategy_task_status",
        description = "Get the current status (phase / error summary) of a previously submitted strategy task."
    )]
    async fn get_strategy_task_status(
        &self,
        Parameters(params): Parameters<GetStrategyTaskStatusParams>,
    ) -> Result<Json<GetStrategyTaskStatusResult>, McpError> {
        self.get_strategy_task_status_inner(params).await.map(Json)
    }

    /// 戦略の多段フェーズ実行設定 (agent_graph YAML) を取得する
    #[tool(
        name = "get_strategy_agent_config",
        description = "Get the multi-phase execution config (agent_graph YAML) for a strategy. Returns an empty string if unset."
    )]
    async fn get_strategy_agent_config(
        &self,
        Parameters(params): Parameters<GetStrategyAgentConfigParams>,
    ) -> Result<Json<GetStrategyAgentConfigResult>, McpError> {
        self.get_strategy_agent_config_inner(params).await.map(Json)
    }

    /// 戦略の多段フェーズ実行設定 (agent_graph YAML) を検証した上で保存する
    #[tool(
        name = "put_strategy_agent_config",
        description = "Validate and save the multi-phase execution config (agent_graph YAML) for a strategy, going through the same validation and history recording as the settings UI. On a validation failure (broken YAML, a duplicate phase key, or a bad for_each reference) this returns ok=false with human-readable error messages instead of failing the tool call, so the caller can read them, fix the YAML, and retry."
    )]
    async fn put_strategy_agent_config(
        &self,
        Parameters(params): Parameters<PutStrategyAgentConfigParams>,
    ) -> Result<Json<PutStrategyAgentConfigResult>, McpError> {
        self.put_strategy_agent_config_inner(params).await.map(Json)
    }

    /// 戦略 id + 件数で最新ノートメタを返す
    #[tool(
        name = "list_recent_notes",
        description = "Return the most recent notes (id / title / status / created_by_kind) for a strategy."
    )]
    async fn list_recent_notes(
        &self,
        Parameters(params): Parameters<ListRecentParams>,
    ) -> Result<Json<ListRecentNotesResult>, McpError> {
        self.list_recent_notes_inner(params).await.map(Json)
    }

    /// RSS フィード一覧
    #[tool(
        name = "list_rss_feeds",
        description = "List RSS feed definitions used by the news aggregator. Pass enabled_only=true to return only feeds currently being polled."
    )]
    async fn list_rss_feeds(
        &self,
        Parameters(params): Parameters<ListRssFeedsParams>,
    ) -> Result<Json<ListRssFeedsResult>, McpError> {
        self.list_rss_feeds_inner(params).await.map(Json)
    }

    /// RSS フィードを追加する
    #[tool(
        name = "create_rss_feed",
        description = "Register a new RSS feed source. The 'source' is a machine slug ([a-z0-9_-]+); 'display_name' is shown to humans. 'url' must be http(s)."
    )]
    async fn create_rss_feed(
        &self,
        Parameters(params): Parameters<CreateRssFeedParams>,
    ) -> Result<Json<RssFeedSummary>, McpError> {
        self.create_rss_feed_inner(params).await.map(Json)
    }

    /// RSS フィードを部分更新する
    #[tool(
        name = "update_rss_feed",
        description = "Update display_name / url / enabled of an existing RSS feed. The source slug is immutable."
    )]
    async fn update_rss_feed(
        &self,
        Parameters(params): Parameters<UpdateRssFeedParams>,
    ) -> Result<Json<RssFeedSummary>, McpError> {
        self.update_rss_feed_inner(params).await.map(Json)
    }

    /// RSS フィードを削除する (既存の news_item 行は残す)
    #[tool(
        name = "delete_rss_feed",
        description = "Delete an RSS feed definition by id. Existing news_item rows are not removed."
    )]
    async fn delete_rss_feed(
        &self,
        Parameters(params): Parameters<DeleteRssFeedParams>,
    ) -> Result<Json<DeleteRssFeedResult>, McpError> {
        self.delete_rss_feed_inner(params).await.map(Json)
    }

    /// 戦略 id + 件数で最新アノテーションメタを返す
    #[tool(
        name = "list_recent_annotations",
        description = "Return the most recent annotations for a strategy."
    )]
    async fn list_recent_annotations(
        &self,
        Parameters(params): Parameters<ListRecentParams>,
    ) -> Result<Json<ListRecentAnnotationsResult>, McpError> {
        self.list_recent_annotations_inner(params).await.map(Json)
    }
}

#[tool_handler]
impl ServerHandler for MgmtServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("t-rader-mgmt", env!("CARGO_PKG_VERSION")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::default(None, 20)]
    #[case::custom(Some(5), 5)]
    #[case::zero_floors_to_one(Some(0), 1)]
    #[case::over_max_caps(Some(1000), 100)]
    fn clamps_limit(#[case] input: Option<u32>, #[case] expected: u64) {
        assert_eq!(clamp_limit(input), expected);
    }
}
