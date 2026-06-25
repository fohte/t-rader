//! 戦略タスクの投入 / 取得を担う service。
//!
//! 管理 MCP (`submit_strategy_task`) と REST (`POST /api/strategies/:id/chat`) の
//! 両方から呼ばれる。戦略 Agent の readiness 確認 → strategy_task 行 (Pending) の
//! 先行 INSERT → kubeopencode Task CR 作成 → 失敗時の Failed への更新までを 1 関数に
//! 集約する。

use chrono::{DateTime, FixedOffset};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::{StrategyAgentStatus, StrategyTaskPhase};
use crate::entities::{strategy, strategy_task};
use crate::kubeopencode::{
    KubeopencodeError, SharedKubeopencodeClient, TaskCrSpec, agent_name_for,
};

/// 戦略タスクの起源。`strategy_task.source` に保存される文字列。
#[derive(Debug, Clone, Copy)]
pub enum TaskSource {
    /// 管理 MCP 経由 (personal-bot / Slack)
    MgmtMcp,
    /// フロントエンド (フローティングチャット)
    Frontend,
    /// cron trigger 発火
    Cron,
    /// hook trigger 発火
    Hook,
}

impl TaskSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskSource::MgmtMcp => "mgmt-mcp",
            TaskSource::Frontend => "frontend",
            TaskSource::Cron => "cron",
            TaskSource::Hook => "hook",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitTaskError {
    #[error("strategy {0} not found")]
    StrategyNotFound(Uuid),
    #[error("prompt must not be empty")]
    EmptyPrompt,
    #[error("strategy agent not ready: status=pending (reconcile in progress)")]
    AgentPending,
    #[error("strategy agent not ready: status=failed: {0}")]
    AgentFailed(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Kubeopencode(#[from] KubeopencodeError),
}

#[derive(Debug, Clone)]
pub struct SubmittedTask {
    pub task_id: Uuid,
    pub kubeopencode_task_name: String,
}

#[derive(Debug, Clone)]
pub struct TaskStatusView {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub kubeopencode_task_name: String,
    pub source: String,
    pub phase: StrategyTaskPhase,
    pub error_summary: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl From<strategy_task::Model> for TaskStatusView {
    fn from(row: strategy_task::Model) -> Self {
        Self {
            task_id: row.task_id,
            strategy_id: row.strategy_id,
            kubeopencode_task_name: row.kubeopencode_task_name,
            source: row.source,
            phase: row.phase,
            error_summary: row.error_summary,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GetTaskError {
    #[error("strategy task {0} not found")]
    NotFound(Uuid),
    #[error("strategy task {task_id} does not belong to strategy {strategy_id}")]
    StrategyMismatch { task_id: Uuid, strategy_id: Uuid },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

fn format_task_name(strategy_id: Uuid, random_short: &str) -> String {
    let strategy_short = &strategy_id.simple().to_string()[..8];
    format!("t-rader-{strategy_short}-{random_short}")
}

fn generate_task_name(strategy_id: Uuid) -> String {
    let random_short = Uuid::new_v4().simple().to_string()[..8].to_string();
    format_task_name(strategy_id, &random_short)
}

/// 戦略 Agent にタスクを投入する。
///
/// CR 孤児化を避けるため、Pending 行を先に INSERT してから Task CR を作成する。
pub async fn submit_task(
    db: &DatabaseConnection,
    kube: &SharedKubeopencodeClient,
    strategy_id: Uuid,
    prompt: &str,
    source: TaskSource,
) -> Result<SubmittedTask, SubmitTaskError> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(SubmitTaskError::EmptyPrompt);
    }

    let strategy_row = strategy::Entity::find_by_id(strategy_id)
        .one(db)
        .await?
        .ok_or(SubmitTaskError::StrategyNotFound(strategy_id))?;
    match strategy_row.agent_status {
        StrategyAgentStatus::Ready => {}
        StrategyAgentStatus::Pending => return Err(SubmitTaskError::AgentPending),
        StrategyAgentStatus::Failed => {
            let reason = strategy_row.agent_error.unwrap_or_else(|| "unknown".into());
            return Err(SubmitTaskError::AgentFailed(reason));
        }
    }

    let task_id = Uuid::new_v4();
    let task_name = generate_task_name(strategy_id);
    let agent_name = agent_name_for(strategy_id);

    let pending = strategy_task::ActiveModel {
        task_id: Set(task_id),
        strategy_id: Set(strategy_id),
        kubeopencode_task_name: Set(task_name.clone()),
        source: Set(source.as_str().to_string()),
        prompt: Set(prompt.clone()),
        phase: Set(StrategyTaskPhase::Pending),
        error_summary: Set(None),
        created_at: NotSet,
        updated_at: NotSet,
    };
    strategy_task::Entity::insert(pending)
        .exec_without_returning(db)
        .await?;

    if let Err(err) = kube
        .create_task(&TaskCrSpec {
            name: task_name.clone(),
            agent_name,
            description: prompt,
        })
        .await
    {
        tracing::warn!(
            error = %err,
            strategy_id = %strategy_id,
            task = %task_name,
            "kubeopencode create_task failed",
        );
        let failed = strategy_task::ActiveModel {
            task_id: Set(task_id),
            phase: Set(StrategyTaskPhase::Failed),
            error_summary: Set(Some(format!("create_task failed: {err}"))),
            updated_at: Set(chrono::Utc::now().fixed_offset()),
            ..Default::default()
        };
        if let Err(update_err) = failed.update(db).await {
            // 行は Pending のまま残り、polling 側からは「完了しない task」に見える。
            // sweep 機構が無いため、ログから手動 reconcile が必要。
            tracing::error!(
                error = %update_err,
                task = %task_name,
                task_id = %task_id,
                "failed to mark strategy_task as failed; row stuck in pending",
            );
        }
        return Err(SubmitTaskError::Kubeopencode(err));
    }

    Ok(SubmittedTask {
        task_id,
        kubeopencode_task_name: task_name,
    })
}

/// strategy_id と task_id を厳密に突き合わせて strategy_task 行を返す。
/// task が存在しない、または strategy_id が一致しない場合はそれぞれ別エラー。
pub async fn get_task_for_strategy(
    db: &DatabaseConnection,
    strategy_id: Uuid,
    task_id: Uuid,
) -> Result<TaskStatusView, GetTaskError> {
    let row = strategy_task::Entity::find_by_id(task_id)
        .one(db)
        .await?
        .ok_or(GetTaskError::NotFound(task_id))?;
    if row.strategy_id != strategy_id {
        return Err(GetTaskError::StrategyMismatch {
            task_id,
            strategy_id,
        });
    }
    Ok(TaskStatusView::from(row))
}

/// kubeopencode_task_name で strategy_task 行を引く (管理 MCP `get_strategy_task_status` 互換)。
pub async fn get_task_by_name(
    db: &DatabaseConnection,
    kubeopencode_task_name: &str,
) -> Result<Option<TaskStatusView>, sea_orm::DbErr> {
    use sea_orm::{ColumnTrait, QueryFilter};
    let row = strategy_task::Entity::find()
        .filter(strategy_task::Column::KubeopencodeTaskName.eq(kubeopencode_task_name))
        .one(db)
        .await?;
    Ok(row.map(TaskStatusView::from))
}

pub fn phase_str(phase: &StrategyTaskPhase) -> &'static str {
    match phase {
        StrategyTaskPhase::Pending => "pending",
        StrategyTaskPhase::Running => "running",
        StrategyTaskPhase::Completed => "completed",
        StrategyTaskPhase::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn task_name_format() {
        let id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        assert_eq!(
            format_task_name(id, "deadbeef"),
            "t-rader-12345678-deadbeef",
        );
    }

    #[rstest]
    #[case::mgmt_mcp(TaskSource::MgmtMcp, "mgmt-mcp")]
    #[case::frontend(TaskSource::Frontend, "frontend")]
    fn task_source_as_str(#[case] source: TaskSource, #[case] expected: &str) {
        assert_eq!(source.as_str(), expected);
    }
}
