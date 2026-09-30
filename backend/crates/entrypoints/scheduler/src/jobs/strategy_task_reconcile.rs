use std::panic::AssertUnwindSafe;

use chrono::Utc;
use core_application::{agent_task_client::AgentTaskClient, strategy_task::StrategyTaskUseCases};
use futures_util::{FutureExt, StreamExt, stream};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, run_with_state};

const MAX_CONCURRENT_STATUS_FETCHES: usize = 8;

#[derive(Debug, Deserialize, Serialize)]
pub struct StrategyTaskReconcile;

impl TaskHandler for StrategyTaskReconcile {
    const IDENTIFIER: &'static str = "strategy_task_reconcile";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let updated = reconcile_in_flight_tasks(
                    &state.dependencies.strategy_tasks,
                    state.dependencies.agent_task_client.as_ref(),
                )
                .await?;
                if updated > 0 {
                    tracing::info!(updated, "strategy task phases reconciled");
                }
                Ok(())
            },
        )
        .await
    }
}

pub async fn reconcile_in_flight_tasks(
    strategy_tasks: &StrategyTaskUseCases,
    agent_client: &dyn AgentTaskClient,
) -> Result<usize, String> {
    let tasks = strategy_tasks
        .list_in_flight_tasks()
        .await
        .map_err(|error| error.to_string())?;
    let now = Utc::now().fixed_offset();
    let results = stream::iter(tasks)
        .map(|task| async move {
            let task_id = task.task_id;
            let result = AssertUnwindSafe(strategy_tasks.reconcile_task(agent_client, task, now))
                .catch_unwind()
                .await;
            (task_id, result)
        })
        .buffer_unordered(MAX_CONCURRENT_STATUS_FETCHES)
        .collect::<Vec<_>>()
        .await;

    let mut updated = 0;
    let mut failures = Vec::new();
    for (task_id, result) in results {
        match result {
            Ok(Ok(true)) => updated += 1,
            Ok(Ok(false)) => {}
            Ok(Err(error)) => {
                tracing::warn!(
                    error = %error,
                    strategy_task_id = %task_id,
                    "strategy task reconcile failed",
                );
                failures.push(task_id.to_string());
            }
            Err(_) => {
                tracing::warn!(
                    strategy_task_id = %task_id,
                    "strategy task reconcile panicked",
                );
                failures.push(task_id.to_string());
            }
        }
    }

    if failures.is_empty() {
        Ok(updated)
    } else {
        Err(format!(
            "strategy task reconciliation failed for {} task(s)",
            failures.len()
        ))
    }
}
