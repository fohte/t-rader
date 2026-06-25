//! 戦略タスク投入の共通 service。
//!
//! 管理 MCP `submit_strategy_task` と trigger fire の双方から呼ばれる。
//! `strategy_task` 行を Pending で先に作って kubeopencode Task CR を作成し、
//! CR 作成失敗時は同じ行を Failed に更新する。

use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::entities::sea_orm_active_enums::{StrategyAgentStatus, StrategyTaskPhase};
use crate::entities::{strategy, strategy_task};
use crate::kubeopencode::{
    KubeopencodeError, SharedKubeopencodeClient, TaskCrSpec, agent_name_for,
};

#[derive(Debug, thiserror::Error)]
pub enum SubmitStrategyTaskError {
    #[error("strategy {0} not found")]
    StrategyNotFound(Uuid),
    #[error("strategy agent not ready: status=pending (reconcile in progress)")]
    AgentPending,
    #[error("strategy agent not ready: status=failed: {0}")]
    AgentFailed(String),
    #[error("prompt must not be empty")]
    EmptyPrompt,
    #[error("kubeopencode error: {0}")]
    Kube(#[from] KubeopencodeError),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
}

#[derive(Debug, Clone)]
pub struct SubmitStrategyTaskOutcome {
    pub task_id: Uuid,
    pub kubeopencode_task_name: String,
}

/// `t-rader-<strategy_id_short>-<random_short>` の文字列フォーマット部分。テスト容易性のため
/// ランダム部分の生成と分離している。
pub fn format_task_name(strategy_id: Uuid, random_short: &str) -> String {
    let strategy_short = &strategy_id.simple().to_string()[..8];
    format!("t-rader-{strategy_short}-{random_short}")
}

/// kubeopencode_task_name を生成する。
pub fn generate_task_name(strategy_id: Uuid) -> String {
    let random_short = Uuid::new_v4().simple().to_string()[..8].to_string();
    format_task_name(strategy_id, &random_short)
}

/// 戦略タスクを投入する。
///
/// invariant: `create_task` 呼び出し前に Pending 行を必ず挿入する。逆順だと create_task
/// 成功後の DB 書き込み失敗で CR が孤児化する。
pub async fn submit_strategy_task(
    db: &DatabaseConnection,
    kube: &SharedKubeopencodeClient,
    strategy_id: Uuid,
    prompt: String,
    source: &str,
) -> Result<SubmitStrategyTaskOutcome, SubmitStrategyTaskError> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(SubmitStrategyTaskError::EmptyPrompt);
    }

    let strategy_row = strategy::Entity::find_by_id(strategy_id)
        .one(db)
        .await?
        .ok_or(SubmitStrategyTaskError::StrategyNotFound(strategy_id))?;
    match strategy_row.agent_status {
        StrategyAgentStatus::Ready => {}
        StrategyAgentStatus::Pending => return Err(SubmitStrategyTaskError::AgentPending),
        StrategyAgentStatus::Failed => {
            let reason = strategy_row.agent_error.unwrap_or_else(|| "unknown".into());
            return Err(SubmitStrategyTaskError::AgentFailed(reason));
        }
    }

    let task_id = Uuid::new_v4();
    let task_name = generate_task_name(strategy_id);
    let agent_name = agent_name_for(strategy_id);

    let pending = strategy_task::ActiveModel {
        task_id: Set(task_id),
        strategy_id: Set(strategy_id),
        kubeopencode_task_name: Set(task_name.clone()),
        source: Set(source.to_string()),
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
        // 行を Failed に更新する。更新自体が失敗しても元のエラーを優先して返す。
        // 主キー + 更新カラムだけの ActiveModel を組み立てて SELECT を省く。
        let failed = strategy_task::ActiveModel {
            task_id: Set(task_id),
            phase: Set(StrategyTaskPhase::Failed),
            error_summary: Set(Some(format!("create_task failed: {err}"))),
            updated_at: Set(chrono::Utc::now().fixed_offset()),
            ..Default::default()
        };
        if let Err(update_err) = failed.update(db).await {
            tracing::warn!(
                error = %update_err,
                task = %task_name,
                "failed to mark strategy_task as failed",
            );
        }
        return Err(SubmitStrategyTaskError::Kube(err));
    }

    Ok(SubmitStrategyTaskOutcome {
        task_id,
        kubeopencode_task_name: task_name,
    })
}
