use uuid::Uuid;

use crate::agent_task_client::{AgentTaskClient, SubmitAgentTask};

use super::StrategyTaskUseCases;
use super::error::{ReconcileTaskError, ResumeTaskError};
use super::queries::step_to_wire_json;
use super::types::{StrategyTaskPhase, StrategyTaskUpdate, SubmittedTask};

impl StrategyTaskUseCases {
    pub async fn resume(
        &self,
        agent_client: &dyn AgentTaskClient,
        task_id: Uuid,
    ) -> Result<SubmittedTask, ResumeTaskError> {
        self.resume_inner(agent_client, task_id, false).await
    }

    pub async fn auto_resume(
        &self,
        agent_client: &dyn AgentTaskClient,
        task_id: Uuid,
    ) -> Result<SubmittedTask, ResumeTaskError> {
        self.resume_inner(agent_client, task_id, true).await
    }

    pub(super) async fn auto_resume_execution_lost(
        &self,
        agent_client: &dyn AgentTaskClient,
        task: super::types::StrategyTask,
        new_error: Option<String>,
        new_result_text: Option<String>,
        new_steps: Option<serde_json::Value>,
    ) -> Result<bool, ReconcileTaskError> {
        let claim_now = chrono::Utc::now().fixed_offset();
        let claim = self
            .apply_failed_and_claim_resumable(
                &task,
                new_error.clone(),
                new_result_text.clone(),
                new_steps,
                claim_now,
            )
            .await?;
        if !claim.claimed {
            return Ok(claim.changed);
        }

        let task_id = task.task_id;
        let claimed_task = super::types::StrategyTask {
            a2a_task_id: None,
            phase: StrategyTaskPhase::Running,
            error_summary: new_error,
            result_text: new_result_text,
            deadline_at: task.deadline_at,
            auto_resumed_at: Some(claim_now),
            updated_at: claim_now,
            ..task
        };
        match self
            .submit_claimed_task(agent_client, claimed_task, task.deadline_at)
            .await
        {
            Ok(submitted) => tracing::info!(
                task_id = %submitted.task_id,
                a2a_task_id = submitted.a2a_task_id,
                "auto-resumed strategy task lost to agent pod churn",
            ),
            Err(error) => tracing::warn!(
                error = %error,
                task_id = %task_id,
                "auto-resume of execution_lost strategy task failed",
            ),
        }
        Ok(claim.changed)
    }

    async fn resume_inner(
        &self,
        agent_client: &dyn AgentTaskClient,
        task_id: Uuid,
        mark_auto_resumed: bool,
    ) -> Result<SubmittedTask, ResumeTaskError> {
        let task = self
            .repository
            .find_by_id(task_id)
            .await?
            .ok_or(ResumeTaskError::NotFound(task_id))?;
        let claim_now = chrono::Utc::now().fixed_offset();
        let transaction = self.unit_of_work.begin().await?;
        let claimed = self
            .repository
            .claim_resumable(&transaction, task_id, claim_now, mark_auto_resumed)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        if !claimed {
            return Err(ResumeTaskError::NotResumable(task_id, task.phase.as_str()));
        }
        let mut claimed_task = task.clone();
        claimed_task.phase = StrategyTaskPhase::Running;
        claimed_task.a2a_task_id = None;
        claimed_task.updated_at = claim_now;
        if mark_auto_resumed {
            claimed_task.auto_resumed_at = Some(claim_now);
        }

        self.submit_claimed_task(agent_client, claimed_task.clone(), claimed_task.deadline_at)
            .await
    }

    async fn submit_claimed_task(
        &self,
        agent_client: &dyn AgentTaskClient,
        claimed_task: super::types::StrategyTask,
        deadline_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<SubmittedTask, ResumeTaskError> {
        let task_id = claimed_task.task_id;
        let steps = self.repository.list_steps(task_id).await?;
        let resume_steps = steps
            .iter()
            .map(|step| {
                let mut value = step_to_wire_json(step);
                if let serde_json::Value::Object(object) = &mut value {
                    object.insert(
                        "execution_step_id".to_string(),
                        serde_json::json!(step.execution_step_id),
                    );
                }
                value
            })
            .collect::<Vec<_>>();

        let agent_ref = match agent_client
            .submit(SubmitAgentTask {
                task_id,
                strategy_id: claimed_task.strategy_id,
                prompt: claimed_task.prompt.clone(),
                purpose: claimed_task.purpose.clone(),
                resume_steps: (!resume_steps.is_empty()).then_some(resume_steps),
                deadline_at,
                as_of: claimed_task.as_of,
            })
            .await
        {
            Ok(agent_ref) => agent_ref,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    task_id = %task_id,
                    "agent task resume submission failed",
                );
                if let Err(update_error) = self
                    .apply_phase(
                        &claimed_task,
                        StrategyTaskPhase::Failed,
                        Some(format!("agent task resume submission failed: {error}")),
                        claimed_task.result_text.clone(),
                        None,
                        true,
                    )
                    .await
                {
                    tracing::error!(
                        error = %update_error,
                        task_id = %task_id,
                        "failed to record resume submission failure on strategy_task",
                    );
                }
                return Err(ResumeTaskError::AgentTask(error));
            }
        };

        if let Err(error) = self
            .update_task(StrategyTaskUpdate {
                task_id,
                a2a_task_id: Some(Some(agent_ref.task_id.clone())),
                phase: None,
                error_summary: Some(None),
                result_text: Some(None),
                deadline_at: None,
                auto_resumed_at: None,
                updated_at: chrono::Utc::now().fixed_offset(),
            })
            .await
        {
            tracing::error!(
                error = %error,
                task_id = %task_id,
                a2a_task_id = %agent_ref.task_id,
                "resumed strategy task submitted but failed to record a2a_task_id; row orphaned until deadline",
            );
            return Err(error);
        }

        Ok(SubmittedTask {
            task_id,
            a2a_task_id: agent_ref.task_id,
        })
    }

    pub(super) async fn update_task(
        &self,
        task: StrategyTaskUpdate,
    ) -> Result<(), ResumeTaskError> {
        let transaction = self.unit_of_work.begin().await?;
        self.repository.update(&transaction, task).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}
