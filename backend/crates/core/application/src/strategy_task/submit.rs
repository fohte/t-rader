use chrono::SubsecRound;
use uuid::Uuid;

use crate::agent_task_client::{AgentTaskClient, SubmitAgentTask};

use super::error::SubmitTaskError;
use super::types::{
    StrategyTask, StrategyTaskPhase, StrategyTaskUpdate, SubmittedTask, TaskSource,
};
use super::{DEADLINE_DURATION, DEFAULT_PURPOSE, StrategyTaskUseCases};

impl StrategyTaskUseCases {
    pub async fn agent_config_exists(&self, purpose: &str) -> Result<bool, SubmitTaskError> {
        self.repository
            .agent_config_exists(purpose)
            .await
            .map_err(Into::into)
    }

    pub async fn submit_task(
        &self,
        agent_client: &dyn AgentTaskClient,
        strategy_id: Uuid,
        prompt: &str,
        source: TaskSource,
        purpose: Option<String>,
    ) -> Result<SubmittedTask, SubmitTaskError> {
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(SubmitTaskError::EmptyPrompt);
        }
        let purpose = purpose.unwrap_or_else(|| DEFAULT_PURPOSE.to_string());
        if !self.repository.strategy_exists(strategy_id).await? {
            return Err(SubmitTaskError::StrategyNotFound(strategy_id));
        }
        if !self.repository.agent_config_exists(&purpose).await? {
            return Err(SubmitTaskError::PurposeNotFound(purpose));
        }

        let task_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();
        let deadline_at = now + DEADLINE_DURATION;
        let as_of = now.trunc_subsecs(6);
        let task = StrategyTask {
            task_id,
            strategy_id,
            a2a_task_id: None,
            source: source.as_str().to_string(),
            prompt: prompt.clone(),
            phase: StrategyTaskPhase::Pending,
            error_summary: None,
            result_text: None,
            deadline_at,
            purpose: Some(purpose.clone()),
            as_of: Some(as_of),
            auto_resumed_at: None,
            created_at: now,
            updated_at: now,
        };
        let transaction = self.unit_of_work.begin().await?;
        self.repository.insert(&transaction, task).await?;
        self.unit_of_work.commit(transaction).await?;

        let agent_ref = match agent_client
            .submit(SubmitAgentTask {
                strategy_id,
                prompt,
                purpose: Some(purpose),
                resume_steps: None,
                deadline_at,
                as_of: Some(as_of),
            })
            .await
        {
            Ok(agent_ref) => agent_ref,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    strategy_id = %strategy_id,
                    task_id = %task_id,
                    "agent task submission failed",
                );
                let update_result = self
                    .update(StrategyTaskUpdate {
                        task_id,
                        a2a_task_id: None,
                        phase: Some(StrategyTaskPhase::Failed),
                        error_summary: Some(Some(format!("agent task submission failed: {error}"))),
                        result_text: None,
                        deadline_at: None,
                        auto_resumed_at: None,
                        updated_at: chrono::Utc::now().fixed_offset(),
                    })
                    .await;
                if let Err(update_error) = update_result {
                    tracing::error!(
                        error = %update_error,
                        task_id = %task_id,
                        "failed to mark strategy_task as failed; row stuck in pending",
                    );
                }
                return Err(SubmitTaskError::AgentTask(error));
            }
        };

        if let Err(error) = self
            .update(StrategyTaskUpdate {
                task_id,
                a2a_task_id: Some(Some(agent_ref.task_id.clone())),
                phase: Some(StrategyTaskPhase::Running),
                error_summary: None,
                result_text: None,
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
                "agent task submitted but failed to record a2a_task_id; row orphaned until deadline",
            );
            return Err(error);
        }

        Ok(SubmittedTask {
            task_id,
            a2a_task_id: agent_ref.task_id,
        })
    }

    async fn update(&self, task: StrategyTaskUpdate) -> Result<(), SubmitTaskError> {
        let transaction = self.unit_of_work.begin().await?;
        self.repository.update(&transaction, task).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}
