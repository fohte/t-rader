use std::{
    panic::AssertUnwindSafe,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use chrono::Utc;
use futures_util::{FutureExt, StreamExt, lock::Mutex, stream};

use crate::agent_task_client::SharedAgentTaskClient;

use super::{
    StrategyTaskReconcileLockGuard, StrategyTaskReconcileLockUseCases, StrategyTaskUseCases,
};

const MAX_CONCURRENT_STATUS_FETCHES: usize = 8;

#[derive(Clone)]
pub struct StrategyTaskReconcileUseCases {
    strategy_tasks: StrategyTaskUseCases,
    lock: StrategyTaskReconcileLockUseCases,
    agent_task_client: SharedAgentTaskClient,
    waiting_reconcile: Arc<WaitingReconcileState>,
}

struct WaitingReconcileState {
    gate: Mutex<()>,
    pending: AtomicBool,
}

impl StrategyTaskReconcileUseCases {
    pub fn new(
        strategy_tasks: StrategyTaskUseCases,
        lock: StrategyTaskReconcileLockUseCases,
        agent_task_client: SharedAgentTaskClient,
    ) -> Self {
        Self {
            strategy_tasks,
            lock,
            agent_task_client,
            waiting_reconcile: Arc::new(WaitingReconcileState {
                gate: Mutex::new(()),
                pending: AtomicBool::new(false),
            }),
        }
    }

    pub async fn reconcile(&self) -> Result<usize, String> {
        let guard =
            self.lock.try_acquire().await.map_err(|error| {
                format!("failed to acquire strategy task reconcile lock: {error}")
            })?;
        let Some(guard) = guard else {
            tracing::info!(
                "strategy task reconciliation skipped because another run holds the lock"
            );
            return Ok(0);
        };

        self.reconcile_with_guard(guard).await
    }

    pub async fn reconcile_waiting_for_lock(&self) -> Result<usize, String> {
        self.waiting_reconcile.pending.store(true, Ordering::SeqCst);
        let _guard = self.waiting_reconcile.gate.lock().await;
        let mut updated = 0;
        let mut last_error = None;

        while self.waiting_reconcile.pending.swap(false, Ordering::SeqCst) {
            match self.reconcile_after_acquiring_lock().await {
                Ok(count) => {
                    updated += count;
                    last_error = None;
                }
                Err(error) => last_error = Some(error),
            }
        }

        last_error.map_or(Ok(updated), Err)
    }

    async fn reconcile_after_acquiring_lock(&self) -> Result<usize, String> {
        let guard =
            self.lock.acquire().await.map_err(|error| {
                format!("failed to acquire strategy task reconcile lock: {error}")
            })?;
        self.reconcile_with_guard(guard).await
    }

    async fn reconcile_with_guard(
        &self,
        guard: Box<dyn StrategyTaskReconcileLockGuard>,
    ) -> Result<usize, String> {
        let reconcile_result =
            reconcile_in_flight_tasks(&self.strategy_tasks, self.agent_task_client.as_ref()).await;
        let release_result = guard
            .release()
            .await
            .map_err(|error| format!("failed to release strategy task reconcile lock: {error}"));

        match (reconcile_result, release_result) {
            (Ok(updated), Ok(())) => Ok(updated),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(reconcile_error), Err(release_error)) => {
                Err(format!("{reconcile_error}; {release_error}"))
            }
        }
    }
}

async fn reconcile_in_flight_tasks(
    strategy_tasks: &StrategyTaskUseCases,
    agent_task_client: &dyn crate::agent_task_client::AgentTaskClient,
) -> Result<usize, String> {
    let tasks = strategy_tasks
        .list_in_flight_tasks()
        .await
        .map_err(|error| error.to_string())?;
    let now = Utc::now().fixed_offset();
    let results = stream::iter(tasks)
        .map(|task| async move {
            let task_id = task.task_id;
            let result =
                AssertUnwindSafe(strategy_tasks.reconcile_task(agent_task_client, task, now))
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
