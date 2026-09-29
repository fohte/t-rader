use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::agent_task_client::{
    AgentTaskClient, AgentTaskError, AgentTaskState, AgentTaskStatus, EXECUTION_LOST_ERROR_KIND,
};

use super::StrategyTaskUseCases;
use super::error::ReconcileTaskError;
use super::types::{StrategyTask, StrategyTaskPhase, StrategyTaskUpdate};

impl StrategyTaskUseCases {
    pub async fn apply_agent_status(
        &self,
        task_id: Uuid,
        status: AgentTaskStatus,
        now: DateTime<FixedOffset>,
    ) -> Result<bool, ReconcileTaskError> {
        let Some(task) = self.repository.find_by_id(task_id).await? else {
            return Ok(false);
        };
        self.apply_status(task, status, now).await
    }

    pub async fn reconcile_task(
        &self,
        agent_client: &dyn AgentTaskClient,
        task: StrategyTask,
        now: DateTime<FixedOffset>,
    ) -> Result<bool, ReconcileTaskError> {
        let Some(a2a_task_id) = task.a2a_task_id.clone() else {
            if now > task.deadline_at {
                return self
                    .apply_failed(task, "agent task submission was not recorded".to_string())
                    .await;
            }
            return Ok(false);
        };

        match agent_client.get(&a2a_task_id).await {
            Ok(status) => {
                let auto_resume_eligible = is_auto_resume_eligible(&task, &status, now);
                let task_id = task.task_id;
                let updated = self.apply_status(task, status, now).await?;
                if updated && auto_resume_eligible {
                    match self.auto_resume(agent_client, task_id).await {
                        Ok(submitted) => tracing::info!(
                            task_id = %task_id,
                            a2a_task_id = submitted.a2a_task_id,
                            "auto-resumed strategy task lost to agent pod churn",
                        ),
                        Err(error) => tracing::warn!(
                            error = %error,
                            task_id = %task_id,
                            "auto-resume of execution_lost strategy task failed; leaving it failed",
                        ),
                    }
                }
                Ok(updated)
            }
            Err(error) => {
                if now > task.deadline_at {
                    let message = match &error {
                        AgentTaskError::NotFound(_) => {
                            format!("agent task {a2a_task_id} not found")
                        }
                        _ => format!("agent task unreachable: {error}"),
                    };
                    self.apply_failed(task, message).await
                } else {
                    tracing::warn!(
                        error = %error,
                        task_id = %task.task_id,
                        a2a_task_id,
                        "failed to fetch agent task status; will retry on next tick",
                    );
                    Ok(false)
                }
            }
        }
    }

    async fn apply_status(
        &self,
        task: StrategyTask,
        status: AgentTaskStatus,
        now: DateTime<FixedOffset>,
    ) -> Result<bool, ReconcileTaskError> {
        let (new_phase, new_error) = if now > task.deadline_at {
            let message = match agent_reported_reason(&status) {
                Some(reported) => {
                    format!("agent task exceeded deadline (agent reported: {reported})")
                }
                None => "agent task exceeded deadline".to_string(),
            };
            (StrategyTaskPhase::Failed, Some(message))
        } else {
            let new_phase = phase_for_state(status.state);
            let new_error = error_summary_for(&status, new_phase);
            (new_phase, new_error)
        };
        let new_result_text = status.result_text.or_else(|| task.result_text.clone());
        self.apply_phase(task, new_phase, new_error, new_result_text, status.steps)
            .await
    }

    async fn apply_failed(
        &self,
        task: StrategyTask,
        message: String,
    ) -> Result<bool, ReconcileTaskError> {
        self.apply_phase(task, StrategyTaskPhase::Failed, Some(message), None, None)
            .await
    }

    async fn apply_phase(
        &self,
        task: StrategyTask,
        new_phase: StrategyTaskPhase,
        new_error: Option<String>,
        new_result_text: Option<String>,
        new_steps: Option<serde_json::Value>,
    ) -> Result<bool, ReconcileTaskError> {
        let row_changed = new_phase != task.phase
            || new_error != task.error_summary
            || new_result_text != task.result_text;
        let task_update = row_changed.then(|| StrategyTaskUpdate {
            task_id: task.task_id,
            a2a_task_id: None,
            phase: Some(new_phase),
            error_summary: Some(new_error),
            result_text: Some(new_result_text),
            deadline_at: None,
            auto_resumed_at: None,
            updated_at: chrono::Utc::now().fixed_offset(),
        });

        let transaction = self.unit_of_work.begin().await?;
        let updated = self
            .repository
            .apply_status_and_steps(&transaction, task.task_id, task_update, new_steps)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }
}

fn phase_for_state(state: AgentTaskState) -> StrategyTaskPhase {
    match state {
        AgentTaskState::Submitted | AgentTaskState::Working => StrategyTaskPhase::Running,
        AgentTaskState::Completed => StrategyTaskPhase::Completed,
        AgentTaskState::InputRequired
        | AgentTaskState::Canceled
        | AgentTaskState::Failed
        | AgentTaskState::Rejected => StrategyTaskPhase::Failed,
    }
}

fn error_summary_for(status: &AgentTaskStatus, phase: StrategyTaskPhase) -> Option<String> {
    if phase != StrategyTaskPhase::Failed {
        return None;
    }
    Some(
        agent_reported_reason(status)
            .unwrap_or("agent task failed")
            .to_string(),
    )
}

fn agent_reported_reason(status: &AgentTaskStatus) -> Option<&str> {
    status
        .error_message
        .as_deref()
        .filter(|message| !message.is_empty())
        .or(status.error_kind.as_deref())
}

fn is_auto_resume_eligible(
    task: &StrategyTask,
    status: &AgentTaskStatus,
    now: DateTime<FixedOffset>,
) -> bool {
    task.auto_resumed_at.is_none()
        && now <= task.deadline_at
        && phase_for_state(status.state) == StrategyTaskPhase::Failed
        && status.error_kind.as_deref() == Some(EXECUTION_LOST_ERROR_KIND)
}
