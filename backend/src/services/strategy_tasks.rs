//! 戦略タスクの投入 / 取得を担う service。
//!
//! 管理 MCP (`submit_strategy_task`)、REST (`POST /api/strategies/:id/chat`)、cron trigger、
//! hook trigger の 4 経路から呼ばれる。strategy_task 行 (Pending) の先行 INSERT →
//! t-rader-agent 内部 API への投入 → 失敗時の Failed への更新までを 1 関数に集約する。

use chrono::{DateTime, FixedOffset};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::agent_client::{AgentTaskError, SharedAgentTaskClient, SubmitAgentTask};
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::{strategy, strategy_task};

/// 内部 API 投入後、client 側で完了を待つ猶予期間。
///
/// t-rader-agent の watchdog (デフォルト 10 分) より長く設定し、working 固着時は
/// watchdog による failed 遷移 (+ push 通知) が先に効くようにする。この deadline は
/// watchdog が機能しない (server ごと長期停止する) 場合の最終防衛。
pub const DEADLINE_DURATION: chrono::Duration = chrono::Duration::minutes(15);

/// 戦略タスクの起源。`strategy_task.source` に保存される文字列。
#[derive(Debug, Clone, Copy)]
pub enum TaskSource {
    /// 管理 MCP 経由 (上流のコントロールプレーン)
    MgmtMcp,
    /// フロントエンド (フローティングチャット)
    Frontend,
    /// cron trigger 発火
    Cron,
    /// hook trigger 発火
    Hook,
    /// ノート / アノテーションのレビュー却下
    Review,
}

impl TaskSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskSource::MgmtMcp => "mgmt-mcp",
            TaskSource::Frontend => "frontend",
            TaskSource::Cron => "cron",
            TaskSource::Hook => "hook",
            TaskSource::Review => "review",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubmitTaskError {
    #[error("strategy {0} not found")]
    StrategyNotFound(Uuid),
    #[error("prompt must not be empty")]
    EmptyPrompt,
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    AgentTask(#[from] AgentTaskError),
}

#[derive(Debug, Clone)]
pub struct SubmittedTask {
    pub task_id: Uuid,
    pub a2a_task_id: String,
}

#[derive(Debug, Clone)]
pub struct TaskStatusView {
    pub task_id: Uuid,
    pub strategy_id: Uuid,
    pub a2a_task_id: Option<String>,
    pub source: String,
    pub prompt: String,
    pub phase: StrategyTaskPhase,
    pub error_summary: Option<String>,
    pub result_text: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub steps: serde_json::Value,
}

impl From<strategy_task::Model> for TaskStatusView {
    fn from(row: strategy_task::Model) -> Self {
        Self {
            task_id: row.task_id,
            strategy_id: row.strategy_id,
            a2a_task_id: row.a2a_task_id,
            source: row.source,
            prompt: row.prompt,
            phase: row.phase,
            error_summary: row.error_summary,
            result_text: row.result_text,
            created_at: row.created_at,
            updated_at: row.updated_at,
            steps: row.steps,
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

/// 戦略 Agent にタスクを投入する。
///
/// Pending 行を先に INSERT してから t-rader-agent 内部 API に投入する。投入成功後は
/// 行に `a2a_task_id` を記録して phase を Running に進める。投入失敗時は行を Failed に
/// 更新する。
pub async fn submit_task(
    db: &DatabaseConnection,
    agent_client: &SharedAgentTaskClient,
    strategy_id: Uuid,
    prompt: &str,
    source: TaskSource,
) -> Result<SubmittedTask, SubmitTaskError> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(SubmitTaskError::EmptyPrompt);
    }

    strategy::Entity::find_by_id(strategy_id)
        .one(db)
        .await?
        .ok_or(SubmitTaskError::StrategyNotFound(strategy_id))?;

    let task_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    let deadline_at = now + DEADLINE_DURATION;

    let pending = strategy_task::ActiveModel {
        task_id: Set(task_id),
        strategy_id: Set(strategy_id),
        a2a_task_id: Set(None),
        source: Set(source.as_str().to_string()),
        prompt: Set(prompt.clone()),
        phase: Set(StrategyTaskPhase::Pending),
        error_summary: Set(None),
        result_text: Set(None),
        deadline_at: Set(deadline_at),
        steps: Set(serde_json::json!([])),
        created_at: NotSet,
        updated_at: NotSet,
    };
    strategy_task::Entity::insert(pending)
        .exec_without_returning(db)
        .await?;

    let agent_ref = match agent_client
        .submit(SubmitAgentTask {
            strategy_id,
            prompt,
        })
        .await
    {
        Ok(agent_ref) => agent_ref,
        Err(err) => {
            tracing::warn!(
                error = %err,
                strategy_id = %strategy_id,
                task_id = %task_id,
                "agent task submission failed",
            );
            let failed = strategy_task::ActiveModel {
                task_id: Set(task_id),
                phase: Set(StrategyTaskPhase::Failed),
                error_summary: Set(Some(format!("agent task submission failed: {err}"))),
                updated_at: Set(chrono::Utc::now().fixed_offset()),
                ..Default::default()
            };
            if let Err(update_err) = failed.update(db).await {
                // 行は Pending のまま残り、watcher 側からは「完了しない task」に見える。
                // deadline 超過で最終的には failed 確定されるが、ログから早期発見できるようにする。
                tracing::error!(
                    error = %update_err,
                    task_id = %task_id,
                    "failed to mark strategy_task as failed; row stuck in pending",
                );
            }
            return Err(SubmitTaskError::AgentTask(err));
        }
    };

    let running = strategy_task::ActiveModel {
        task_id: Set(task_id),
        a2a_task_id: Set(Some(agent_ref.task_id.clone())),
        phase: Set(StrategyTaskPhase::Running),
        updated_at: Set(chrono::Utc::now().fixed_offset()),
        ..Default::default()
    };
    if let Err(err) = running.update(db).await {
        // agent への投入自体は成功しているため、この行は a2a_task_id が記録されない
        // まま孤児化する。watcher が deadline 超過で failed 確定するが、実際には
        // agent 側でタスクが動いているので、追跡できるよう a2a_task_id をログに残す。
        tracing::error!(
            error = %err,
            task_id = %task_id,
            a2a_task_id = %agent_ref.task_id,
            "agent task submitted but failed to record a2a_task_id; row orphaned until deadline",
        );
        return Err(SubmitTaskError::Database(err));
    }

    Ok(SubmittedTask {
        task_id,
        a2a_task_id: agent_ref.task_id,
    })
}

/// 一覧取得時の上限件数。ページネーションは今のところ無く、直近分だけを返す。
const TASK_LIST_LIMIT: u64 = 50;

/// 戦略の過去タスクを新しい順に返す (最大 `TASK_LIST_LIMIT` 件)。
pub async fn list_tasks_for_strategy(
    db: &DatabaseConnection,
    strategy_id: Uuid,
) -> Result<Vec<TaskStatusView>, sea_orm::DbErr> {
    use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect};
    let rows = strategy_task::Entity::find()
        .filter(strategy_task::Column::StrategyId.eq(strategy_id))
        .order_by_desc(strategy_task::Column::CreatedAt)
        .limit(TASK_LIST_LIMIT)
        .all(db)
        .await?;
    Ok(rows.into_iter().map(TaskStatusView::from).collect())
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

/// a2a_task_id で strategy_task 行を引く (管理 MCP `get_strategy_task_status` 互換)。
pub async fn get_task_by_a2a_task_id(
    db: &DatabaseConnection,
    a2a_task_id: &str,
) -> Result<Option<TaskStatusView>, sea_orm::DbErr> {
    use sea_orm::{ColumnTrait, QueryFilter};
    let row = strategy_task::Entity::find()
        .filter(strategy_task::Column::A2aTaskId.eq(a2a_task_id))
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

    #[rstest]
    #[case::mgmt_mcp(TaskSource::MgmtMcp, "mgmt-mcp")]
    #[case::frontend(TaskSource::Frontend, "frontend")]
    #[case::review(TaskSource::Review, "review")]
    fn task_source_as_str(#[case] source: TaskSource, #[case] expected: &str) {
        assert_eq!(source.as_str(), expected);
    }
}
