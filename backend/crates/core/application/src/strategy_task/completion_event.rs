use serde_json::json;

use crate::event::NewEvent;

use super::StrategyTaskUseCases;
use super::error::StrategyTaskCompletionError;
use super::repository::StrategyTaskApplyResult;
use super::types::{StrategyTask, StrategyTaskPhase, StrategyTaskUpdate};

impl StrategyTaskUseCases {
    pub(super) async fn apply_phase(
        &self,
        task: &StrategyTask,
        new_phase: StrategyTaskPhase,
        new_error: Option<String>,
        new_result_text: Option<String>,
        new_steps: Option<serde_json::Value>,
        emit_failure_event: bool,
    ) -> Result<StrategyTaskApplyResult, StrategyTaskCompletionError> {
        let row_changed = new_phase != task.phase
            || new_error != task.error_summary
            || new_result_text != task.result_text;
        let task_update = row_changed.then(|| StrategyTaskUpdate {
            task_id: task.task_id,
            a2a_task_id: None,
            phase: Some(new_phase),
            error_summary: Some(new_error.clone()),
            result_text: Some(new_result_text.clone()),
            deadline_at: None,
            auto_resumed_at: None,
            updated_at: chrono::Utc::now().fixed_offset(),
        });

        let transaction = self.unit_of_work.begin().await?;
        let result = self
            .repository
            .apply_status_and_steps(
                &transaction,
                task.task_id,
                task.a2a_task_id.as_deref(),
                task_update,
                new_steps,
            )
            .await?;

        let should_enqueue_dispatch =
            if new_phase == StrategyTaskPhase::Failed && !emit_failure_event {
                false
            } else {
                self.create_completion_event(
                    &transaction,
                    task,
                    new_phase,
                    result.task_updated,
                    &new_error,
                    &new_result_text,
                )
                .await?
            };

        self.unit_of_work.commit(transaction).await?;
        if should_enqueue_dispatch
            && let Some(completion_events) = self.completion_events.as_ref()
            && let Err(error) = completion_events.dispatch_job.enqueue_dispatch().await
        {
            tracing::warn!(
                error = %error,
                task_id = %task.task_id,
                "failed to enqueue event dispatch; periodic dispatch will pick up the committed event",
            );
        }

        Ok(result)
    }

    async fn create_completion_event(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        task: &StrategyTask,
        new_phase: StrategyTaskPhase,
        task_updated: bool,
        new_error: &Option<String>,
        new_result_text: &Option<String>,
    ) -> Result<bool, StrategyTaskCompletionError> {
        if !task_updated {
            return Ok(false);
        }
        let Some(event_type) = event_type_for_phase(new_phase) else {
            return Ok(false);
        };
        let Some(completion_events) = self.completion_events.as_ref() else {
            return Ok(false);
        };

        let source_chain_depth = completion_events
            .repository
            .find_chain_depth_for_task(transaction, task.task_id)
            .await?;
        completion_events
            .repository
            .create(
                transaction,
                NewEvent {
                    event_type: event_type.to_string(),
                    payload: json!({
                        "task_id": task.task_id,
                        "strategy_id": task.strategy_id,
                        "purpose": task.purpose,
                        "result_text": new_result_text,
                        "error_summary": new_error,
                    }),
                    chain_depth: Some(
                        source_chain_depth.map_or(0, |depth| depth.saturating_add(1)),
                    ),
                },
            )
            .await?;
        Ok(true)
    }
}

fn event_type_for_phase(phase: StrategyTaskPhase) -> Option<&'static str> {
    match phase {
        StrategyTaskPhase::Completed => Some("strategy_task.completed"),
        StrategyTaskPhase::Failed => Some("strategy_task.failed"),
        StrategyTaskPhase::Pending | StrategyTaskPhase::Running => None,
    }
}
