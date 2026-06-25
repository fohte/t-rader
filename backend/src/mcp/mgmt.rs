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

use crate::entities::{annotation, note, strategy};
use crate::kubeopencode::{KubeopencodeError, SharedKubeopencodeClient};
use crate::services::strategy_tasks::{
    self, SubmitTaskError, TaskSource, TaskStatusView, phase_str,
};

const DEFAULT_LIST_LIMIT: u64 = 20;
const MAX_LIST_LIMIT: u64 = 100;

#[derive(Clone)]
pub struct MgmtServer {
    db: DatabaseConnection,
    kube: SharedKubeopencodeClient,
}

impl MgmtServer {
    pub fn new(db: DatabaseConnection, kube: SharedKubeopencodeClient) -> Self {
        Self { db, kube }
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
    pub kubeopencode_task_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetStrategyTaskStatusParams {
    pub kubeopencode_task_name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetStrategyTaskStatusResult {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub kubeopencode_task_name: String,
    pub phase: String,
    pub error_summary: Option<String>,
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

    /// 戦略 id + prompt から kubeopencode Task CR を作成し task_name を返す
    #[tool(
        name = "submit_strategy_task",
        description = "Submit a strategy task: create a kubeopencode Task CR for the strategy agent and return the task name."
    )]
    async fn submit_strategy_task(
        &self,
        Parameters(params): Parameters<SubmitStrategyTaskParams>,
    ) -> Result<Json<SubmitStrategyTaskResult>, McpError> {
        let submitted = strategy_tasks::submit_task(
            &self.db,
            &self.kube,
            params.strategy_id,
            &params.prompt,
            TaskSource::MgmtMcp,
        )
        .await
        .map_err(map_submit_error)?;
        Ok(Json(SubmitStrategyTaskResult {
            task_id: submitted.task_id,
            kubeopencode_task_name: submitted.kubeopencode_task_name,
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
            strategy_tasks::get_task_by_name(&self.db, &params.kubeopencode_task_name)
                .await
                .map_err(db_error)?
                .ok_or_else(|| McpError::resource_not_found("strategy task not found", None))?;
        Ok(Json(GetStrategyTaskStatusResult {
            task_id: view.task_id,
            strategy_id: view.strategy_id,
            kubeopencode_task_name: view.kubeopencode_task_name,
            phase: phase_str(&view.phase).to_string(),
            error_summary: view.error_summary,
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
        SubmitTaskError::AgentPending => {
            invalid_params("strategy agent not ready: status=pending (reconcile in progress)")
        }
        SubmitTaskError::AgentFailed(reason) => {
            invalid_params(format!("strategy agent not ready: status=failed: {reason}"))
        }
        SubmitTaskError::Database(db_err) => db_error(db_err),
        SubmitTaskError::Kubeopencode(kube_err) => map_kube_error(&kube_err),
    }
}

fn map_kube_error(err: &KubeopencodeError) -> McpError {
    match err {
        KubeopencodeError::NotConfigured => internal_error("kubeopencode is not configured"),
        KubeopencodeError::AlreadyExists(name) => {
            invalid_params(format!("kubeopencode task already exists: {name}"))
        }
        KubeopencodeError::NotFound(name) => {
            McpError::resource_not_found(format!("kubeopencode task not found: {name}"), None)
        }
        KubeopencodeError::Api { status, message } => internal_error(format!(
            "kubeopencode api error (status {status}): {message}"
        )),
        KubeopencodeError::Network(msg) => {
            internal_error(format!("kubeopencode network error: {msg}"))
        }
        KubeopencodeError::Parse(msg) => internal_error(format!("kubeopencode parse error: {msg}")),
        KubeopencodeError::Init(msg) => internal_error(format!("kubeopencode init error: {msg}")),
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
    use sea_orm::IntoActiveModel;
    use sqlx::PgPool;

    use crate::entities::sea_orm_active_enums::{StrategyAgentStatus, StrategyTaskPhase};
    use crate::entities::strategy_task;
    use crate::kubeopencode::{FakeKubeopencodeClient, KubeopencodeError, agent_name_for};
    use crate::testing::create_test_db;

    use super::*;

    /// 管理 MCP 経由で投入されたタスクの `strategy_task.source` 値。
    const MGMT_TASK_SOURCE: &str = "mgmt-mcp";

    async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
        insert_strategy_with_status(db, name, StrategyAgentStatus::Ready).await
    }

    async fn insert_strategy_with_status(
        db: &DatabaseConnection,
        name: &str,
        status: StrategyAgentStatus,
    ) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: sea_orm::ActiveValue::NotSet,
            skills: sea_orm::ActiveValue::NotSet,
            agent_status: Set(status),
            agent_error: sea_orm::ActiveValue::NotSet,
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(db)
        .await
        .unwrap();
        id
    }

    fn build_server(db: DatabaseConnection, fake: Arc<FakeKubeopencodeClient>) -> MgmtServer {
        MgmtServer::new(db, fake as SharedKubeopencodeClient)
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_inserts_row_and_creates_cr(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long-term").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        let server = build_server(db.clone(), fake.clone());

        let Json(result) = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: " inspect 7203 ".into(),
            }))
            .await
            .expect("submit ok");

        let task_name = result.kubeopencode_task_name.clone();
        let task_id = result.task_id;

        // kubeopencode に渡された TaskCrSpec の集合は、戻り値の task_name を採用したもの 1 件
        let created: Vec<(String, String, String)> = fake
            .created
            .lock()
            .await
            .iter()
            .map(|s| (s.name.clone(), s.agent_name.clone(), s.description.clone()))
            .collect();
        assert_eq!(
            created,
            vec![(
                task_name.clone(),
                agent_name_for(strategy_id),
                "inspect 7203".to_string(),
            )],
        );

        // strategy_task 行は pending で 1 件、戻り値と一致する内容を持つ
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
                kubeopencode_task_name: task_name,
                source: MGMT_TASK_SOURCE.to_string(),
                prompt: "inspect 7203".to_string(),
                phase: StrategyTaskPhase::Pending,
                error_summary: None,
            }],
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    struct StrategyTaskRowSummary {
        task_id: Uuid,
        strategy_id: Uuid,
        kubeopencode_task_name: String,
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
                kubeopencode_task_name: m.kubeopencode_task_name,
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
        let fake = Arc::new(FakeKubeopencodeClient::new());
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
    async fn submit_strategy_task_rejects_pending_agent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy_with_status(&db, "x", StrategyAgentStatus::Pending).await;
        let server = build_server(db, Arc::new(FakeKubeopencodeClient::new()));
        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "x".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "strategy agent not ready: status=pending (reconcile in progress)",
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_rejects_failed_agent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy_with_status(&db, "x", StrategyAgentStatus::Failed).await;
        // 失敗時の agent_error を反映できることを確認するため、別途 UPDATE する
        let mut row = strategy::Entity::find_by_id(strategy_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .into_active_model();
        row.agent_error = Set(Some("boom".into()));
        row.update(&db).await.unwrap();

        let server = build_server(db, Arc::new(FakeKubeopencodeClient::new()));
        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "x".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "strategy agent not ready: status=failed: boom",
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_rejects_empty_prompt(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db, Arc::new(FakeKubeopencodeClient::new()));
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
    async fn submit_strategy_task_persists_failure_on_kube_error(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
        fake.set_create_error(KubeopencodeError::Api {
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
            Some("create_task failed: kubeopencode api error (status 500): boom"),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_status_returns_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeKubeopencodeClient::new());
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
                kubeopencode_task_name: submitted.kubeopencode_task_name.clone(),
            }))
            .await
            .expect("ok");
        assert_eq!(status.task_id, submitted.task_id);
        assert_eq!(status.strategy_id, strategy_id);
        assert_eq!(status.phase, "pending");
        assert!(status.error_summary.is_none());
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_status_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeKubeopencodeClient::new()));
        let err = server
            .get_strategy_task_status(Parameters(GetStrategyTaskStatusParams {
                kubeopencode_task_name: "nonexistent".into(),
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

        let server = build_server(db, Arc::new(FakeKubeopencodeClient::new()));
        let Json(result) = server.list_strategies().await.expect("ok");
        assert_eq!(result.strategies.len(), 1);
        assert_eq!(result.strategies[0].strategy_id, strategy_id);
        assert_eq!(result.strategies[0].unread_card_count, 3);
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
            }
            .insert(&db)
            .await
            .unwrap();
        }
        let server = build_server(db, Arc::new(FakeKubeopencodeClient::new()));
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
