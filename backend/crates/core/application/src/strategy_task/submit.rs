use chrono::SubsecRound;
use uuid::Uuid;

use crate::agent_task_client::{AgentTaskClient, SubmitAgentTask};
use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::SubmitTaskError;
use super::types::{
    PreparedStrategyTaskSubmission, StrategyTask, StrategyTaskContext, StrategyTaskPhase,
    StrategyTaskUpdate, SubmittedTask, TaskSource,
};
use super::{DEFAULT_PURPOSE, StrategyTaskUseCases, deadline_duration_from_agent_graph};

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
        self.submit_task_with_context(agent_client, strategy_id, prompt, source, purpose, None)
            .await
    }

    pub async fn submit_task_with_context(
        &self,
        agent_client: &dyn AgentTaskClient,
        strategy_id: Uuid,
        prompt: &str,
        source: TaskSource,
        purpose: Option<String>,
        context: Option<StrategyTaskContext>,
    ) -> Result<SubmittedTask, SubmitTaskError> {
        let prepared = self
            .prepare_submission(strategy_id, prompt, source, purpose, context)
            .await?;
        let transaction = self.unit_of_work.begin().await?;
        self.insert_prepared_submission(&transaction, &prepared)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        self.submit_prepared_submission(agent_client, prepared)
            .await
    }

    pub(crate) async fn prepare_submission(
        &self,
        strategy_id: Uuid,
        prompt: &str,
        source: TaskSource,
        purpose: Option<String>,
        context: Option<StrategyTaskContext>,
    ) -> Result<PreparedStrategyTaskSubmission, SubmitTaskError> {
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(SubmitTaskError::EmptyPrompt);
        }
        let purpose = purpose.unwrap_or_else(|| DEFAULT_PURPOSE.to_string());
        if !self.repository.strategy_exists(strategy_id).await? {
            return Err(SubmitTaskError::StrategyNotFound(strategy_id));
        }
        let agent_graph = self
            .repository
            .find_agent_graph_by_purpose(&purpose)
            .await?
            .ok_or_else(|| SubmitTaskError::PurposeNotFound(purpose.clone()))?;
        let deadline_duration = deadline_duration_from_agent_graph(&agent_graph);

        let task_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();
        let deadline_at = now + deadline_duration;
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
            context_kind: context.as_ref().map(|context| context.kind.clone()),
            context_id: context.map(|context| context.id),
            auto_resumed_at: None,
            created_at: now,
            updated_at: now,
        };
        Ok(PreparedStrategyTaskSubmission {
            task,
            request: SubmitAgentTask {
                task_id,
                strategy_id,
                prompt,
                purpose: Some(purpose),
                resume_steps: None,
                deadline_at,
                as_of: Some(as_of),
            },
        })
    }

    pub(crate) async fn insert_prepared_submission(
        &self,
        transaction: &UnitOfWorkTransaction,
        prepared: &PreparedStrategyTaskSubmission,
    ) -> Result<(), SubmitTaskError> {
        self.repository
            .insert(transaction, prepared.task.clone())
            .await?;
        if let Some(execution_budget) = &self.execution_budget {
            execution_budget
                .initialize_task(
                    transaction,
                    prepared.task.task_id,
                    prepared.task.deadline_at,
                )
                .await?;
        }
        if let Some(snapshotter) = &self.task_start_snapshotter {
            snapshotter
                .capture_task_start(
                    transaction,
                    prepared.task.task_id,
                    prepared.task.purpose.as_deref().unwrap_or(DEFAULT_PURPOSE),
                )
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn submit_prepared_submission(
        &self,
        agent_client: &dyn AgentTaskClient,
        prepared: PreparedStrategyTaskSubmission,
    ) -> Result<SubmittedTask, SubmitTaskError> {
        let task_id = prepared.task.task_id;
        let agent_ref = match agent_client.submit(prepared.request).await {
            Ok(agent_ref) => agent_ref,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    strategy_id = %prepared.task.strategy_id,
                    task_id = %task_id,
                    "agent task submission failed",
                );
                let update_result = self
                    .apply_phase(
                        &prepared.task,
                        StrategyTaskPhase::Failed,
                        Some(format!("agent task submission failed: {error}")),
                        prepared.task.result_text.clone(),
                        None,
                        true,
                    )
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
