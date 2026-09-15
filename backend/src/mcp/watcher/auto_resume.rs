//! execution_lost で failed になった戦略タスクの自動 resume 判定・実行。

use chrono::{DateTime, FixedOffset};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::agent_client::{AgentTaskStatus, EXECUTION_LOST_ERROR_KIND, SharedAgentTaskClient};
use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
use crate::entities::strategy_task;
use crate::services::strategy_tasks;

use super::phase_for_state;

/// 自動 resume の対象かどうかを判定する。deadline 超過・既に自動 resume 済み・
/// error_kind が execution_lost 以外のいずれかに該当すれば対象外。
pub(super) fn is_eligible(
    row: &strategy_task::Model,
    status: &AgentTaskStatus,
    now: DateTime<FixedOffset>,
) -> bool {
    row.auto_resumed_at.is_none()
        && now <= row.deadline_at
        && phase_for_state(status.state) == StrategyTaskPhase::Failed
        && status.error_kind.as_deref() == Some(EXECUTION_LOST_ERROR_KIND)
}

/// 自動 resume を試みる。投入自体が失敗しても呼び出し元は再試行しない
/// (claim 時点で auto_resumed_at が刻まれるため、次回以降は is_eligible が false になる)。
pub(super) async fn attempt(
    db: &DatabaseConnection,
    agent_client: &SharedAgentTaskClient,
    task_id: Uuid,
) {
    match strategy_tasks::auto_resume_task(db, agent_client, task_id).await {
        Ok(submitted) => {
            tracing::info!(
                task_id = %task_id,
                a2a_task_id = submitted.a2a_task_id,
                "auto-resumed strategy task lost to agent pod churn",
            );
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                task_id = %task_id,
                "auto-resume of execution_lost strategy task failed; leaving it failed",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::agent_client::AgentTaskState;

    use super::*;

    fn default_row(now: DateTime<FixedOffset>) -> strategy_task::Model {
        strategy_task::Model {
            task_id: Uuid::new_v4(),
            strategy_id: Uuid::new_v4(),
            a2a_task_id: Some("a2a-task".to_string()),
            source: "review".to_string(),
            prompt: "p".to_string(),
            phase: StrategyTaskPhase::Running,
            error_summary: None,
            result_text: None,
            deadline_at: now + chrono::Duration::minutes(15),
            purpose: None,
            as_of: None,
            auto_resumed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn default_status() -> AgentTaskStatus {
        AgentTaskStatus {
            state: AgentTaskState::Failed,
            result_text: None,
            error_kind: Some(EXECUTION_LOST_ERROR_KIND.to_string()),
            steps: None,
        }
    }

    #[rstest]
    #[case::eligible(
        |_row: &mut strategy_task::Model, _status: &mut AgentTaskStatus| {},
        true
    )]
    #[case::already_auto_resumed(
        |row: &mut strategy_task::Model, _status: &mut AgentTaskStatus| {
            row.auto_resumed_at = Some(chrono::Utc::now().fixed_offset());
        },
        false
    )]
    #[case::past_deadline(
        |row: &mut strategy_task::Model, _status: &mut AgentTaskStatus| {
            row.deadline_at -= chrono::Duration::hours(1);
        },
        false
    )]
    #[case::non_execution_lost_error_kind(
        |_row: &mut strategy_task::Model, status: &mut AgentTaskStatus| {
            status.error_kind = Some("usage_limit".to_string());
        },
        false
    )]
    #[case::state_not_mapped_to_failed(
        |_row: &mut strategy_task::Model, status: &mut AgentTaskStatus| {
            status.state = AgentTaskState::Working;
        },
        false
    )]
    fn is_eligible_cases(
        #[case] mutate: fn(&mut strategy_task::Model, &mut AgentTaskStatus),
        #[case] expected: bool,
    ) {
        let now = chrono::Utc::now().fixed_offset();
        let mut row = default_row(now);
        let mut status = default_status();
        mutate(&mut row, &mut status);

        assert_eq!(is_eligible(&row, &status, now), expected);
    }
}
