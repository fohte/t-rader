//! 管理 MCP server の tool 実装
//!
//! personal-bot から呼び出される。tool は以下の 5 種:
//!
//! - `list_strategies`
//! - `submit_strategy_task`
//! - `get_strategy_task_status`
//! - `list_recent_notes`
//! - `list_recent_annotations`

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset};
use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent_client::{AgentTaskError, SharedAgentTaskClient};
use crate::entities::{annotation, note, rss_feed, strategy};
use crate::services::rss_feed as rss_feed_svc;
use crate::services::strategy_tasks::{
    self, SubmitTaskError, TaskSource, TaskStatusView, phase_str,
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

// === tool 入出力のスキーマ ===

#[derive(Debug, Serialize, JsonSchema)]
pub struct StrategySummary {
    pub strategy_id: Uuid,
    pub name: String,
    pub updated_at: DateTime<FixedOffset>,
    /// status='unread' のノート + アノテーション件数の合計
    pub unread_card_count: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListStrategiesResult {
    pub strategies: Vec<StrategySummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitStrategyTaskParams {
    pub strategy_id: Uuid,
    pub prompt: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SubmitStrategyTaskResult {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStrategyTaskStatusParams {
    pub a2a_task_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetStrategyTaskStatusResult {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub phase: String,
    pub error_summary: Option<String>,
    pub result_text: Option<String>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecentParams {
    pub strategy_id: Uuid,
    /// 取得件数 (デフォルト 20、最大 100)
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NoteMeta {
    pub note_id: Uuid,
    pub title: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecentNotesResult {
    pub notes: Vec<NoteMeta>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AnnotationMeta {
    pub annotation_id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub status: String,
    pub created_by_kind: String,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecentAnnotationsResult {
    pub annotations: Vec<AnnotationMeta>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRssFeedsParams {
    /// true なら enabled=true の行のみ返す
    #[serde(default)]
    pub enabled_only: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RssFeedSummary {
    pub id: Uuid,
    pub source: String,
    pub display_name: String,
    pub url: String,
    pub enabled: bool,
}

impl From<rss_feed::Model> for RssFeedSummary {
    fn from(m: rss_feed::Model) -> Self {
        Self {
            id: m.id,
            source: m.source,
            display_name: m.display_name,
            url: m.url,
            enabled: m.enabled,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRssFeedsResult {
    pub feeds: Vec<RssFeedSummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateRssFeedParams {
    pub source: String,
    pub display_name: String,
    pub url: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateRssFeedParams {
    pub id: Uuid,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteRssFeedParams {
    pub id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteRssFeedResult {
    pub id: Uuid,
}

// === MCP error マッピング ===

fn internal_error(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::internal_error(msg, None)
}

fn invalid_params(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::invalid_params(msg, None)
}

fn db_error(err: sea_orm::DbErr) -> McpError {
    tracing::error!(error = %err, "mgmt mcp db error");
    internal_error(format!("database error: {err}"))
}

fn clamp_limit(limit: Option<u32>) -> u64 {
    let value = limit.map(u64::from).unwrap_or(DEFAULT_LIST_LIMIT);
    value.clamp(1, MAX_LIST_LIMIT)
}

/// 指定エンティティの `status='unread'` 件数を strategy_id ごとに集約して返す。
/// `list_strategies` が note / annotation 双方に対し 1 クエリで未読件数を取るために使う。
async fn unread_counts_by_strategy<E, C>(
    db: &DatabaseConnection,
    strategy_id_col: C,
    status_col: C,
    id_col: C,
) -> Result<HashMap<Uuid, u64>, McpError>
where
    E: EntityTrait,
    C: ColumnTrait,
{
    let rows: Vec<(Uuid, i64)> = E::find()
        .select_only()
        .column(strategy_id_col)
        .column_as(id_col.count(), "unread_count")
        .filter(status_col.eq("unread"))
        .group_by(strategy_id_col)
        .into_tuple()
        .all(db)
        .await
        .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|(sid, c)| (sid, c.max(0) as u64))
        .collect())
}

#[tool_router]
impl MgmtServer {
    /// 戦略一覧 (id / 名前 / 最終更新 / 未読カード数)
    #[tool(
        name = "list_strategies",
        description = "List all strategies with id, name, last updated time, and unread card count."
    )]
    async fn list_strategies(&self) -> Result<Json<ListStrategiesResult>, McpError> {
        let rows = strategy::Entity::find()
            .order_by_asc(strategy::Column::SortOrder)
            .order_by_asc(strategy::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        let note_counts = unread_counts_by_strategy::<note::Entity, note::Column>(
            &self.db,
            note::Column::StrategyId,
            note::Column::Status,
            note::Column::Id,
        )
        .await?;
        let annotation_counts =
            unread_counts_by_strategy::<annotation::Entity, annotation::Column>(
                &self.db,
                annotation::Column::StrategyId,
                annotation::Column::Status,
                annotation::Column::Id,
            )
            .await?;

        let strategies = rows
            .into_iter()
            .map(|row| {
                let note_unread = note_counts.get(&row.id).copied().unwrap_or(0);
                let annotation_unread = annotation_counts.get(&row.id).copied().unwrap_or(0);
                StrategySummary {
                    strategy_id: row.id,
                    name: row.name,
                    updated_at: row.updated_at,
                    unread_card_count: note_unread + annotation_unread,
                }
            })
            .collect();
        Ok(Json(ListStrategiesResult { strategies }))
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
        let submitted = strategy_tasks::submit_task(
            &self.db,
            &self.agent_client,
            params.strategy_id,
            &params.prompt,
            TaskSource::MgmtMcp,
        )
        .await
        .map_err(map_submit_error)?;
        Ok(Json(SubmitStrategyTaskResult {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        }))
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
        let view: TaskStatusView =
            strategy_tasks::get_task_by_a2a_task_id(&self.db, &params.a2a_task_id)
                .await
                .map_err(db_error)?
                .ok_or_else(|| McpError::resource_not_found("strategy task not found", None))?;
        Ok(Json(GetStrategyTaskStatusResult {
            task_id: view.task_id,
            strategy_id: view.strategy_id,
            a2a_task_id: view.a2a_task_id,
            phase: phase_str(&view.phase).to_string(),
            error_summary: view.error_summary,
            result_text: view.result_text,
            updated_at: view.updated_at,
        }))
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
        let limit = clamp_limit(params.limit);
        let rows = note::Entity::find()
            .filter(note::Column::StrategyId.eq(params.strategy_id))
            .order_by_desc(note::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        let notes = rows
            .into_iter()
            .map(|row| NoteMeta {
                note_id: row.id,
                title: row.title,
                status: row.status,
                created_by_kind: row.created_by_kind,
                updated_at: row.updated_at,
            })
            .collect();
        Ok(Json(ListRecentNotesResult { notes }))
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
        let rows = rss_feed_svc::list(&self.db, params.enabled_only.unwrap_or(false))
            .await
            .map_err(map_rss_feed_error)?;
        Ok(Json(ListRssFeedsResult {
            feeds: rows.into_iter().map(Into::into).collect(),
        }))
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
        Ok(Json(created.into()))
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
        Ok(Json(updated.into()))
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
        rss_feed_svc::delete(&self.db, params.id)
            .await
            .map_err(map_rss_feed_error)?;
        Ok(Json(DeleteRssFeedResult { id: params.id }))
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
        let limit = clamp_limit(params.limit);
        let rows = annotation::Entity::find()
            .filter(annotation::Column::StrategyId.eq(params.strategy_id))
            .order_by_desc(annotation::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        let annotations = rows
            .into_iter()
            .map(|row| AnnotationMeta {
                annotation_id: row.id,
                target_symbol: row.target_symbol,
                target_kind: row.target_kind,
                status: row.status,
                created_by_kind: row.created_by_kind,
                updated_at: row.updated_at,
            })
            .collect();
        Ok(Json(ListRecentAnnotationsResult { annotations }))
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

fn map_submit_error(err: SubmitTaskError) -> McpError {
    match err {
        SubmitTaskError::EmptyPrompt => invalid_params("prompt must not be empty"),
        SubmitTaskError::StrategyNotFound(id) => invalid_params(format!("strategy {id} not found")),
        SubmitTaskError::Database(db_err) => db_error(db_err),
        SubmitTaskError::AgentTask(agent_err) => map_agent_task_error(&agent_err),
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
        rss_feed_svc::RssFeedError::Database(e) => db_error(e),
    }
}

fn map_agent_task_error(err: &AgentTaskError) -> McpError {
    match err {
        AgentTaskError::NotConfigured => internal_error("agent task client is not configured"),
        AgentTaskError::NotFound(name) => {
            McpError::resource_not_found(format!("agent task not found: {name}"), None)
        }
        AgentTaskError::Api { status, message } => {
            internal_error(format!("agent task api error (status {status}): {message}"))
        }
        AgentTaskError::Network(msg) => internal_error(format!("agent task network error: {msg}")),
        AgentTaskError::Parse(msg) => internal_error(format!("agent task parse error: {msg}")),
        AgentTaskError::Init(msg) => internal_error(format!("agent task init error: {msg}")),
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

#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;

    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sqlx::PgPool;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::strategy_task;
    use crate::testing::create_test_db;

    use super::*;

    /// 管理 MCP 経由で投入されたタスクの `strategy_task.source` 値。
    const MGMT_TASK_SOURCE: &str = "mgmt-mcp";

    async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: sea_orm::ActiveValue::NotSet,
            skills: sea_orm::ActiveValue::NotSet,
            agent_graph: sea_orm::ActiveValue::NotSet,
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    fn build_server(db: DatabaseConnection, fake: Arc<FakeAgentTaskClient>) -> MgmtServer {
        MgmtServer::new(db, fake as SharedAgentTaskClient)
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_inserts_row_and_submits_to_agent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long-term").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-1").await;
        let server = build_server(db.clone(), fake.clone());

        let Json(result) = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: " inspect 7203 ".into(),
            }))
            .await
            .expect("submit ok");

        assert_eq!(result.a2a_task_id, "agent-task-1");
        let task_id = result.task_id;

        // agent_client に渡された投入内容の集合は、戻り値の task_id を採用したもの 1 件
        let submitted: Vec<(uuid::Uuid, String)> = fake
            .submitted
            .lock()
            .await
            .iter()
            .map(|s| (s.strategy_id, s.prompt.clone()))
            .collect();
        assert_eq!(submitted, vec![(strategy_id, "inspect 7203".to_string())]);

        // strategy_task 行は running で 1 件、戻り値と一致する内容を持つ
        let rows: Vec<StrategyTaskRowSummary> = strategy_task::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(StrategyTaskRowSummary::from_model)
            .collect();
        assert_eq!(
            rows,
            vec![StrategyTaskRowSummary {
                task_id,
                strategy_id,
                a2a_task_id: Some("agent-task-1".to_string()),
                source: MGMT_TASK_SOURCE.to_string(),
                prompt: "inspect 7203".to_string(),
                phase: StrategyTaskPhase::Running,
                error_summary: None,
            }],
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    struct StrategyTaskRowSummary {
        task_id: Uuid,
        strategy_id: Uuid,
        a2a_task_id: Option<String>,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
        error_summary: Option<String>,
    }

    impl StrategyTaskRowSummary {
        fn from_model(m: strategy_task::Model) -> Self {
            Self {
                task_id: m.task_id,
                strategy_id: m.strategy_id,
                a2a_task_id: m.a2a_task_id,
                source: m.source,
                prompt: m.prompt,
                phase: m.phase,
                error_summary: m.error_summary,
            }
        }
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_rejects_unknown_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let server = build_server(db, fake);

        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id: Uuid::new_v4(),
                prompt: "x".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_rejects_empty_prompt(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "   ".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_persists_failure_on_agent_error(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::Api {
            status: 500,
            message: "boom".into(),
        })
        .await;
        let server = build_server(db.clone(), fake);

        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "x".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);

        let rows = strategy_task::Entity::find().all(&db).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].phase, StrategyTaskPhase::Failed);
        assert_eq!(
            rows[0].error_summary.as_deref(),
            Some("agent task submission failed: agent task api error (status 500): boom"),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_status_returns_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let server = build_server(db.clone(), fake);

        let Json(submitted) = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "p".into(),
            }))
            .await
            .expect("submit");

        let Json(status) = server
            .get_strategy_task_status(Parameters(GetStrategyTaskStatusParams {
                a2a_task_id: submitted.a2a_task_id.clone(),
            }))
            .await
            .expect("ok");
        assert_eq!(status.task_id, submitted.task_id);
        assert_eq!(status.strategy_id, strategy_id);
        assert_eq!(status.phase, "running");
        assert!(status.error_summary.is_none());
        assert!(status.result_text.is_none());
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_status_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let err = server
            .get_strategy_task_status(Parameters(GetStrategyTaskStatusParams {
                a2a_task_id: "nonexistent".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_strategies_counts_unread_cards(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;

        // unread ノート 2 件、approved ノート 1 件 → unread だけカウント
        for (title, status) in [("a", "unread"), ("b", "unread"), ("c", "approved")] {
            note::ActiveModel {
                id: Set(Uuid::new_v4()),
                strategy_id: Set(strategy_id),
                title: Set(title.to_string()),
                body_md: Set("body".to_string()),
                frontmatter_json: Set(serde_json::json!({})),
                type_tag: Set(None),
                status: Set(status.to_string()),
                trigger: Set(None),
                trigger_label: Set(None),
                created_by_kind: Set("human".to_string()),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
                graphs_json: Set(serde_json::json!([])),
            }
            .insert(&db)
            .await
            .unwrap();
        }
        // unread アノテーション 1 件
        annotation::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            target_symbol: Set("7203".into()),
            target_kind: Set("signal".into()),
            timestamp: Set(chrono::Utc::now().fixed_offset()),
            price: Set(None),
            text: Set("note".into()),
            status: Set("unread".into()),
            linked_note_id: Set(None),
            created_by_kind: Set("llm".into()),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .unwrap();

        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let Json(result) = server.list_strategies().await.expect("ok");
        assert_eq!(result.strategies.len(), 1);
        assert_eq!(result.strategies[0].strategy_id, strategy_id);
        assert_eq!(result.strategies[0].unread_card_count, 3);
    }

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

    #[sqlx::test(migrations = false)]
    async fn list_recent_notes_caps_by_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        for i in 0..5 {
            note::ActiveModel {
                id: Set(Uuid::new_v4()),
                strategy_id: Set(strategy_id),
                title: Set(format!("note-{i}")),
                body_md: Set("body".into()),
                frontmatter_json: Set(serde_json::json!({})),
                type_tag: Set(None),
                status: Set("unread".into()),
                trigger: Set(None),
                trigger_label: Set(None),
                created_by_kind: Set("human".into()),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
                graphs_json: Set(serde_json::json!([])),
            }
            .insert(&db)
            .await
            .unwrap();
        }
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let Json(result) = server
            .list_recent_notes(Parameters(ListRecentParams {
                strategy_id,
                limit: Some(3),
            }))
            .await
            .expect("ok");
        assert_eq!(result.notes.len(), 3);
    }
}
