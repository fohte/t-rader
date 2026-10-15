use std::{future::Future, panic::AssertUnwindSafe};

use chrono::Utc;
use core_application::{
    agent_task_client::AgentTaskClient,
    strategy_task::{StrategyTaskReconcileLockUseCases, StrategyTaskUseCases},
};
use futures_util::{FutureExt, StreamExt, stream};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;

use super::{DAILY_TIMEOUT, run_with_state};

const MAX_CONCURRENT_STATUS_FETCHES: usize = 8;

#[derive(Debug, Deserialize, Serialize)]
#[serde(from = "Option<EmptyObject>")]
pub struct StrategyTaskReconcile;

// webhook は {}、cron は payload 未指定時に null を渡す。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyObject {}

impl From<Option<EmptyObject>> for StrategyTaskReconcile {
    fn from(_payload: Option<EmptyObject>) -> Self {
        Self
    }
}

impl TaskHandler for StrategyTaskReconcile {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for StrategyTaskReconcile {
    const IDENTIFIER: &'static str = STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let lock = state.dependencies.strategy_task_reconcile_lock.clone();
                    run_with_reconcile_lock(&lock, move || async move {
                        let reconcile_result = reconcile_in_flight_tasks(
                            &state.dependencies.strategy_tasks,
                            state.dependencies.agent_task_client.as_ref(),
                        )
                        .await
                        .map(|updated| {
                            if updated > 0 {
                                tracing::info!(updated, "strategy task phases reconciled");
                            }
                        });
                        let dispatch_result =
                            super::event_dispatch::dispatch_pending_events(&state)
                                .await
                                .map_err(|error| {
                                    format!("failed to dispatch pending events: {error}")
                                });

                        combine_task_results(reconcile_result, dispatch_result)
                    })
                    .await
                },
            )
            .await
        })
    }
}

async fn run_with_reconcile_lock<F, Fut>(
    lock: &StrategyTaskReconcileLockUseCases,
    work: F,
) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let guard = lock
        .try_acquire()
        .await
        .map_err(|error| format!("failed to acquire strategy task reconcile lock: {error}"))?;
    let Some(guard) = guard else {
        tracing::info!("strategy task reconciliation skipped because another run holds the lock");
        return Ok(());
    };

    let work_result = work().await;
    let release_result = guard
        .release()
        .await
        .map_err(|error| format!("failed to release strategy task reconcile lock: {error}"));

    combine_task_results(work_result, release_result)
}

fn combine_task_results(
    first_result: Result<(), String>,
    second_result: Result<(), String>,
) -> Result<(), String> {
    match (first_result, second_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(first_error), Err(second_error)) => Err(format!("{first_error}; {second_error}")),
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use core_application::{
        agent_task_client::{AgentTaskState, AgentTaskStatus, FakeAgentTaskClient},
        persistence::PersistenceError,
        strategy_task::{
            StrategyTask, StrategyTaskOrigin, StrategyTaskPhase, StrategyTaskReconcileLock,
            StrategyTaskReconcileLockGuard, StrategyTaskReconcileLockUseCases,
            StrategyTaskRepository, StrategyTaskRepositoryError, StrategyTaskStep,
            StrategyTaskUpdate, StrategyTaskUseCases, TaskListQuery,
        },
        unit_of_work::FakeUnitOfWork,
    };
    use rstest::rstest;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::{StrategyTaskReconcile, reconcile_in_flight_tasks, run_with_reconcile_lock};

    #[rstest]
    #[case::cron_payload_null(serde_json::Value::Null, Ok(()))]
    #[case::webhook_payload_empty_object(serde_json::json!({}), Ok(()))]
    #[case::unexpected_field_is_rejected(serde_json::json!({"unexpected": true}), Err(()))]
    fn deserializes_reconcile_job_payloads(
        #[case] payload: serde_json::Value,
        #[case] expected: Result<(), ()>,
    ) {
        assert_eq!(
            serde_json::from_value::<StrategyTaskReconcile>(payload)
                .map(|_| ())
                .map_err(|_| ()),
            expected,
        );
    }

    struct FakeReconcileLock {
        available: bool,
        acquired: Arc<AtomicUsize>,
        released: Arc<AtomicUsize>,
        held: Arc<AtomicBool>,
    }

    struct FakeReconcileLockGuard {
        released: Arc<AtomicUsize>,
        held: Arc<AtomicBool>,
    }

    #[async_trait]
    impl StrategyTaskReconcileLock for FakeReconcileLock {
        async fn try_acquire(
            &self,
        ) -> Result<Option<Box<dyn StrategyTaskReconcileLockGuard>>, PersistenceError> {
            self.acquired.fetch_add(1, Ordering::SeqCst);
            Ok(self.available.then(|| {
                self.held.store(true, Ordering::SeqCst);
                Box::new(FakeReconcileLockGuard {
                    released: Arc::clone(&self.released),
                    held: Arc::clone(&self.held),
                }) as Box<dyn StrategyTaskReconcileLockGuard>
            }))
        }
    }

    #[async_trait]
    impl StrategyTaskReconcileLockGuard for FakeReconcileLockGuard {
        async fn release(self: Box<Self>) -> Result<(), PersistenceError> {
            self.held.store(false, Ordering::SeqCst);
            self.released.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[rstest]
    #[case::lock_busy(false, Ok(()), Ok(()), 0)]
    #[case::lock_acquired(true, Ok(()), Ok(()), 1)]
    #[case::work_failed(
        true,
        Err("reconciliation failed".to_string()),
        Err("reconciliation failed".to_string()),
        1,
    )]
    #[tokio::test]
    async fn runs_work_only_while_holding_the_reconcile_lock(
        #[case] available: bool,
        #[case] work_result: Result<(), String>,
        #[case] expected_result: Result<(), String>,
        #[case] expected_work_runs: usize,
    ) {
        let acquired = Arc::new(AtomicUsize::new(0));
        let released = Arc::new(AtomicUsize::new(0));
        let work_runs = Arc::new(AtomicUsize::new(0));
        let held = Arc::new(AtomicBool::new(false));
        let held_during_work = Arc::new(AtomicBool::new(false));
        let lock = StrategyTaskReconcileLockUseCases::new(Arc::new(FakeReconcileLock {
            available,
            acquired: Arc::clone(&acquired),
            released: Arc::clone(&released),
            held: Arc::clone(&held),
        }));
        let work_runs_in_future = Arc::clone(&work_runs);
        let held_in_future = Arc::clone(&held);
        let held_during_work_in_future = Arc::clone(&held_during_work);
        let work_result_in_future = work_result.clone();

        let result = run_with_reconcile_lock(&lock, move || async move {
            work_runs_in_future.fetch_add(1, Ordering::SeqCst);
            held_during_work_in_future
                .store(held_in_future.load(Ordering::SeqCst), Ordering::SeqCst);
            work_result_in_future
        })
        .await;

        assert_eq!(
            (
                result,
                acquired.load(Ordering::SeqCst),
                released.load(Ordering::SeqCst),
                work_runs.load(Ordering::SeqCst),
                held_during_work.load(Ordering::SeqCst),
                held.load(Ordering::SeqCst),
            ),
            (
                expected_result,
                1,
                expected_work_runs,
                expected_work_runs,
                available,
                false,
            ),
        );
    }

    struct PartialFailureRepository {
        tasks: Vec<StrategyTask>,
        failed_task_id: Uuid,
        updated_task_ids: Mutex<Vec<Uuid>>,
    }

    #[async_trait]
    impl StrategyTaskRepository for PartialFailureRepository {
        async fn strategy_exists(
            &self,
            _strategy_id: Uuid,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(false)
        }

        async fn agent_config_exists(
            &self,
            _purpose: &str,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(false)
        }

        async fn find_agent_graph_by_purpose(
            &self,
            _purpose: &str,
        ) -> Result<Option<String>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn insert(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            _task: StrategyTask,
        ) -> Result<(), StrategyTaskRepositoryError> {
            Ok(())
        }

        async fn update(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            _task: StrategyTaskUpdate,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(false)
        }

        async fn apply_status_and_steps(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            task_id: Uuid,
            _expected_a2a_task_id: Option<&str>,
            _task_update: Option<StrategyTaskUpdate>,
            _steps: Option<serde_json::Value>,
        ) -> Result<
            core_application::strategy_task::StrategyTaskApplyResult,
            StrategyTaskRepositoryError,
        > {
            if task_id == self.failed_task_id {
                return Err(PersistenceError::Database("write failed".to_string()).into());
            }
            self.updated_task_ids.lock().await.push(task_id);
            Ok(core_application::strategy_task::StrategyTaskApplyResult {
                task_updated: true,
                steps_updated: false,
            })
        }

        async fn find_by_id(
            &self,
            _task_id: Uuid,
        ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn find_by_a2a_task_id(
            &self,
            _a2a_task_id: &str,
        ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn find_origin_by_execution_step_id(
            &self,
            _execution_step_id: Uuid,
        ) -> Result<Option<StrategyTaskOrigin>, StrategyTaskRepositoryError> {
            Ok(None)
        }

        async fn list(
            &self,
            _query: TaskListQuery,
        ) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(Vec::new())
        }

        async fn list_in_flight(&self) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
            Ok(self.tasks.clone())
        }

        async fn list_steps(
            &self,
            _task_id: Uuid,
        ) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError> {
            Ok(Vec::new())
        }

        async fn claim_resumable(
            &self,
            _transaction: &core_application::unit_of_work::UnitOfWorkTransaction,
            _task_id: Uuid,
            _now: chrono::DateTime<chrono::FixedOffset>,
            _deadline_at: chrono::DateTime<chrono::FixedOffset>,
            _mark_auto_resumed: bool,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            Ok(false)
        }
    }

    fn task(task_id: Uuid, a2a_task_id: &str) -> StrategyTask {
        let now = Utc::now().fixed_offset();
        StrategyTask {
            task_id,
            strategy_id: Uuid::new_v4(),
            a2a_task_id: Some(a2a_task_id.to_string()),
            source: "test".to_string(),
            prompt: "prompt".to_string(),
            phase: StrategyTaskPhase::Running,
            error_summary: None,
            result_text: None,
            deadline_at: now + Duration::minutes(1),
            purpose: None,
            as_of: None,
            context_kind: None,
            context_id: None,
            auto_resumed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn reports_partial_failure_after_reconciling_other_tasks() {
        let failed_task_id = Uuid::new_v4();
        let updated_task_id = Uuid::new_v4();
        let repository = Arc::new(PartialFailureRepository {
            tasks: vec![
                task(failed_task_id, "external-task-a"),
                task(updated_task_id, "external-task-b"),
            ],
            failed_task_id,
            updated_task_ids: Mutex::new(Vec::new()),
        });
        let strategy_tasks =
            StrategyTaskUseCases::new(Arc::new(FakeUnitOfWork::new()), repository.clone());
        let agent_client = FakeAgentTaskClient::new();
        for a2a_task_id in ["external-task-a", "external-task-b"] {
            agent_client
                .set_status(
                    a2a_task_id,
                    AgentTaskStatus {
                        state: AgentTaskState::Completed,
                        result_text: Some("finished".to_string()),
                        error_message: None,
                        error_kind: None,
                        steps: None,
                    },
                )
                .await;
        }

        let result = reconcile_in_flight_tasks(&strategy_tasks, &agent_client).await;
        let mut updated_task_ids = repository.updated_task_ids.lock().await.clone();
        updated_task_ids.sort();

        assert_eq!(
            (result, updated_task_ids),
            (
                Err("strategy task reconciliation failed for 1 task(s)".to_string()),
                vec![updated_task_id],
            ),
        );
    }
}
