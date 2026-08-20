//! 管理 MCP server の tool 実装
//!
//! personal-bot から呼び出される。tool は以下の 16 種:
//!
//! - `list_strategies`
//! - `submit_strategy_task`
//! - `get_strategy_task_status`
//! - `get_strategy_config`
//! - `create_strategy`
//! - `update_strategy_config`
//! - `delete_strategy`
//! - `create_strategy_trigger`
//! - `update_strategy_trigger`
//! - `delete_strategy_trigger`
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
//! - `strategy_config`: 戦略設定 (name/description/agents_md/skills/agent_graph) の
//!   取得・作成・更新・削除と、戦略に紐づく trigger の一覧取得 (読み取り専用)
//!   (`get_strategy_config_inner` / `create_strategy_inner` / `update_strategy_config_inner` /
//!   `delete_strategy_inner`)
//! - `triggers`: trigger の作成・更新・削除
//!   (`create_strategy_trigger_inner` / `update_strategy_trigger_inner` / `delete_strategy_trigger_inner`)
//! - `rss_feeds`: RSS フィード CRUD
//!   (`list_rss_feeds_inner` / `create_rss_feed_inner` / `update_rss_feed_inner` / `delete_rss_feed_inner`)
//! - `notes_annotations`: 直近ノート・アノテーション一覧
//!   (`list_recent_notes_inner` / `list_recent_annotations_inner`)
//!
//! 本モジュールは tool wrapper (`#[tool_router]` / `#[tool_handler]`) と
//! 共通のエラー変換ヘルパを担う。

pub(super) mod dto;
mod notes_annotations;
mod rss_feeds;
mod strategies;
mod strategy_config;
mod triggers;

#[cfg(test)]
mod tests_common;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use sea_orm::DatabaseConnection;

use crate::agent_client::SharedAgentTaskClient;
use crate::error::AppError;

// `SubmitStrategyTaskParams` は integration_tests.rs からも直接参照されるため公開する。
pub use dto::SubmitStrategyTaskParams;
use dto::{
    CreateRssFeedParams, CreateStrategyParams, CreateStrategyResult, CreateStrategyTriggerParams,
    CreateStrategyTriggerResult, DeleteRssFeedParams, DeleteRssFeedResult, DeleteStrategyParams,
    DeleteStrategyResult, DeleteStrategyTriggerParams, DeleteStrategyTriggerResult,
    GetStrategyConfigParams, GetStrategyConfigResult, GetStrategyTaskStatusParams,
    GetStrategyTaskStatusResult, ListRecentAnnotationsResult, ListRecentNotesResult,
    ListRecentParams, ListRssFeedsParams, ListRssFeedsResult, ListStrategiesResult, RssFeedSummary,
    SubmitStrategyTaskResult, UpdateRssFeedParams, UpdateStrategyConfigParams,
    UpdateStrategyConfigResult, UpdateStrategyTriggerParams, UpdateStrategyTriggerResult,
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

/// `AppError::Validation` は各 tool 側で `ok=false` + `errors` として扱うため、ここでは
/// `NotFound` / `Database` / その他だけを tool call の失敗として一律にマッピングする。
pub(super) fn map_app_error(err: AppError) -> McpError {
    match err {
        AppError::NotFound(msg) => invalid_params(msg),
        AppError::Database(db_err) => db_error(db_err),
        other => internal_error(other.to_string()),
    }
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

    /// 戦略の設定 (name/description/agents_md/skills/agent_graph) と trigger 一覧を取得する
    #[tool(
        name = "get_strategy_config",
        description = "Get a strategy's full config: name, description, agents_md, skills, agent_graph, and its triggers."
    )]
    async fn get_strategy_config(
        &self,
        Parameters(params): Parameters<GetStrategyConfigParams>,
    ) -> Result<Json<GetStrategyConfigResult>, McpError> {
        self.get_strategy_config_inner(params).await.map(Json)
    }

    /// 戦略を作成する (name のみ必須、他は任意で 1 回の呼び出しでまとめて設定できる)
    #[tool(
        name = "create_strategy",
        description = "Create a new strategy with name (required) and optionally description, agents_md, skills, and agent_graph in one call. On a validation failure (empty name, a bad skill name, or invalid agent_graph YAML) this returns ok=false with all the errors it found instead of failing the tool call, so the caller can read them, fix the input, and retry; no strategy is created when any error is present."
    )]
    async fn create_strategy(
        &self,
        Parameters(params): Parameters<CreateStrategyParams>,
    ) -> Result<Json<CreateStrategyResult>, McpError> {
        self.create_strategy_inner(params).await.map(Json)
    }

    /// 戦略の設定を部分更新する (1 回の呼び出しで複数フィールドをまとめて atomic に反映)
    #[tool(
        name = "update_strategy_config",
        description = "Update only the given fields of a strategy's config in one atomic call. skills uses JSON Merge Patch semantics: a null value deletes that skill, anything else upserts it, and omitted keys are left unchanged. On a validation failure (a bad skill name or invalid agent_graph YAML) this returns ok=false with all the errors it found instead of failing the tool call; nothing is written when any error is present."
    )]
    async fn update_strategy_config(
        &self,
        Parameters(params): Parameters<UpdateStrategyConfigParams>,
    ) -> Result<Json<UpdateStrategyConfigResult>, McpError> {
        self.update_strategy_config_inner(params).await.map(Json)
    }

    /// 戦略を削除する (confirm_name の完全一致必須、関連リソースは cascade 削除)
    #[tool(
        name = "delete_strategy",
        description = "Delete a strategy and cascade-delete everything under it (notes, annotations, trades, hypotheses, triggers, custom indicators, strategy tasks, interests). confirm_name must exactly match the strategy's current name or nothing is deleted, to guard against a wrong strategy_id."
    )]
    async fn delete_strategy(
        &self,
        Parameters(params): Parameters<DeleteStrategyParams>,
    ) -> Result<Json<DeleteStrategyResult>, McpError> {
        self.delete_strategy_inner(params).await.map(Json)
    }

    /// trigger を作成する
    #[tool(
        name = "create_strategy_trigger",
        description = "Create a trigger for a strategy. kind=cron requires schedule (and forbids hook_slug); kind=hook requires hook_slug (and forbids schedule). On a validation failure this returns ok=false with the first error found (checks stop at the first failure, so a retry may surface a different one) instead of failing the tool call, so the caller can read it, fix the input, and retry; no trigger is created when an error is present. A database-level conflict (e.g. a hook_slug already used by another trigger) fails the tool call instead of returning ok=false."
    )]
    async fn create_strategy_trigger(
        &self,
        Parameters(params): Parameters<CreateStrategyTriggerParams>,
    ) -> Result<Json<CreateStrategyTriggerResult>, McpError> {
        self.create_strategy_trigger_inner(params).await.map(Json)
    }

    /// trigger を部分更新する (kind / strategy_id は不変)
    #[tool(
        name = "update_strategy_trigger",
        description = "Update only the given fields of an existing trigger. kind and strategy_id are immutable; schedule can only be set on a cron trigger and hook_slug only on a hook trigger. On a validation failure this returns ok=false with the first error found (checks stop at the first failure, so a retry may surface a different one) instead of failing the tool call; nothing is written when an error is present. A database-level conflict (e.g. a hook_slug already used by another trigger) fails the tool call instead of returning ok=false."
    )]
    async fn update_strategy_trigger(
        &self,
        Parameters(params): Parameters<UpdateStrategyTriggerParams>,
    ) -> Result<Json<UpdateStrategyTriggerResult>, McpError> {
        self.update_strategy_trigger_inner(params).await.map(Json)
    }

    /// trigger を削除する
    #[tool(
        name = "delete_strategy_trigger",
        description = "Delete a trigger by trigger_id."
    )]
    async fn delete_strategy_trigger(
        &self,
        Parameters(params): Parameters<DeleteStrategyTriggerParams>,
    ) -> Result<Json<DeleteStrategyTriggerResult>, McpError> {
        self.delete_strategy_trigger_inner(params).await.map(Json)
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
