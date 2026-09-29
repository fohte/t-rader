use uuid::Uuid;

use crate::agent_task_client::{AgentTaskClient, SubmitAgentTask};

use super::error::ResumeTaskError;
use super::queries::step_to_wire_json;
use super::types::{StrategyTaskPhase, StrategyTaskUpdate, SubmittedTask};
use super::{DEADLINE_DURATION, StrategyTaskUseCases};

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

        let now = chrono::Utc::now().fixed_offset();
        let deadline_at = now + DEADLINE_DURATION;
        let agent_ref = match agent_client
            .submit(SubmitAgentTask {
                strategy_id: task.strategy_id,
                prompt: task.prompt,
                purpose: task.purpose,
                resume_steps: (!resume_steps.is_empty()).then_some(resume_steps),
                deadline_at,
                as_of: task.as_of,
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
                    .update_task(StrategyTaskUpdate {
                        task_id,
                        a2a_task_id: None,
                        phase: Some(StrategyTaskPhase::Failed),
                        error_summary: Some(Some(format!(
                            "agent task resume submission failed: {error}"
                        ))),
                        result_text: None,
                        deadline_at: None,
                        auto_resumed_at: None,
                        updated_at: chrono::Utc::now().fixed_offset(),
                    })
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
                deadline_at: Some(deadline_at),
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
