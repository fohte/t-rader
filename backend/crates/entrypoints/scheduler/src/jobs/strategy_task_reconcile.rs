use std::panic::AssertUnwindSafe;

use chrono::Utc;
use core_application::{agent_task_client::AgentTaskClient, strategy_task::StrategyTaskUseCases};
use futures_util::{FutureExt, StreamExt, stream};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use core_application::strategy_task::STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;

use super::{DAILY_TIMEOUT, run_with_state};

const MAX_CONCURRENT_STATUS_FETCHES: usize = 8;

#[derive(Debug, Deserialize, Serialize)]
#[serde(from = "StrategyTaskReconcilePayload")]
pub struct StrategyTaskReconcile;

// webhook は {}、cron は payload 未指定時に null を渡す。
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StrategyTaskReconcilePayload {
    Null(()),
    EmptyObject(EmptyObject),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyObject {}

impl From<StrategyTaskReconcilePayload> for StrategyTaskReconcile {
    fn from(_payload: StrategyTaskReconcilePayload) -> Self {
        Self
    }
}

impl TaskHandler for StrategyTaskReconcile {
    const IDENTIFIER: &'static str = STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use core_application::{
        agent_task_client::{AgentTaskState, AgentTaskStatus, FakeAgentTaskClient},
        persistence::PersistenceError,
        strategy_task::{
            StrategyTask, StrategyTaskPhase, StrategyTaskRepository, StrategyTaskRepositoryError,
            StrategyTaskStep, StrategyTaskUpdate, StrategyTaskUseCases, TaskListQuery,
        },
        unit_of_work::FakeUnitOfWork,
    };
    use rstest::rstest;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::{StrategyTaskReconcile, reconcile_in_flight_tasks};

    #[rstest]
    #[case::cron_payload_null(serde_json::Value::Null)]
    #[case::webhook_payload_empty_object(serde_json::json!({}))]
    fn deserializes_reconcile_job_payloads(#[case] payload: serde_json::Value) {
        assert_eq!(
            serde_json::from_value::<StrategyTaskReconcile>(payload)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            Ok(()),
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
            _task_update: Option<StrategyTaskUpdate>,
            _steps: Option<serde_json::Value>,
        ) -> Result<bool, StrategyTaskRepositoryError> {
            if task_id == self.failed_task_id {
                return Err(PersistenceError::Database("write failed".to_string()).into());
            }
            self.updated_task_ids.lock().await.push(task_id);
            Ok(true)
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

        async fn find_strategy_id_by_execution_step_id(
            &self,
            _execution_step_id: Uuid,
        ) -> Result<Option<Uuid>, StrategyTaskRepositoryError> {
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
