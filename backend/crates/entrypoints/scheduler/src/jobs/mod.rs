use std::future::Future;
use std::time::Duration;

use core_application::ingest_run_log::IngestRunLog;
use graphile_worker::WorkerContext;
use serde::Serialize;
use tokio::time::timeout;

use crate::state::SchedulerState;

pub mod fred;
pub mod ingest_run_recovery;
pub mod jquants;
pub mod prediction;

pub(super) const DAILY_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
pub(super) const WEEKLY_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);

pub(super) fn require_source<T>(source: Option<T>, name: &str) -> Result<T, String> {
    source.ok_or_else(|| format!("{name} is not configured"))
}

pub(super) async fn run_with_state<F, Fut, Stats>(
    context: WorkerContext,
    task_name: &'static str,
    timeout: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<Stats, String>>,
    Stats: Serialize,
{
    let Some(state) = context.get_ext::<SchedulerState>() else {
        return Err("scheduler state is not configured".to_string());
    };

    run_with_ingest_run_log(
        state.dependencies.ingest_run_log.as_ref(),
        task_name,
        timeout,
        task(state.clone()),
    )
    .await
}

async fn run_with_ingest_run_log<F, Stats>(
    ingest_run_log: &dyn IngestRunLog,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: Future<Output = Result<Stats, String>>,
    Stats: Serialize,
{
    let run_id = ingest_run_log
        .start(task_name)
        .await
        .map_err(|error| format!("failed to start ingest run log: {error}"))?;

    let task_result = match timeout(duration, task).await {
        Ok(result) => result,
        Err(_) => Err(format!("{task_name} timed out after {duration:?}")),
    };
    let recorded_result = task_result.and_then(|stats| {
        serde_json::to_value(stats)
            .map_err(|error| format!("failed to serialize job stats: {error}"))
    });

    if let Err(log_error) = ingest_run_log.finish(run_id, recorded_result.clone()).await {
        let error = match recorded_result {
            Ok(_) => format!("failed to finish ingest run log: {log_error}"),
            Err(task_error) => {
                format!("{task_error}; failed to finish ingest run log: {log_error}")
            }
        };
        tracing::error!(task = task_name, %error, "failed to persist scheduled job result");
        return Err(error);
    }

    match recorded_result {
        Ok(_) => {
            tracing::info!(task = task_name, "scheduled job completed");
            Ok(())
        }
        Err(error) => {
            tracing::warn!(task = task_name, %error, "scheduled job failed");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{future::pending, sync::Arc, time::Duration};

    use async_trait::async_trait;
    use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
    use serde::Serialize;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::run_with_ingest_run_log;

    const RUN_ID: Uuid = Uuid::from_u128(1);

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Start {
            job: String,
        },
        Finish {
            run_id: Uuid,
            result: Result<Value, String>,
        },
    }

    #[derive(Default)]
    struct FakeIngestRunLog {
        calls: Mutex<Vec<Call>>,
    }

    #[async_trait]
    impl IngestRunLog for FakeIngestRunLog {
        async fn start(&self, job: &str) -> Result<Uuid, PersistenceError> {
            self.calls.lock().await.push(Call::Start {
                job: job.to_string(),
            });
            Ok(RUN_ID)
        }

        async fn finish(
            &self,
            run_id: Uuid,
            result: Result<Value, String>,
        ) -> Result<(), PersistenceError> {
            self.calls
                .lock()
                .await
                .push(Call::Finish { run_id, result });
            Ok(())
        }

        async fn fail_interrupted_before(
            &self,
            _job: &str,
            _started_before: chrono::DateTime<chrono::FixedOffset>,
        ) -> Result<u64, PersistenceError> {
            Ok(0)
        }
    }

    #[derive(Serialize)]
    struct IngestStats {
        rows: usize,
    }

    #[tokio::test]
    async fn records_serialized_stats_when_the_job_succeeds() {
        let log = Arc::new(FakeIngestRunLog::default());

        let result = run_with_ingest_run_log(
            log.as_ref(),
            "sample_ingest",
            Duration::from_secs(1),
            async { Ok(IngestStats { rows: 3 }) },
        )
        .await;

        assert_eq!(
            (result, log.calls.lock().await.clone()),
            (
                Ok(()),
                vec![
                    Call::Start {
                        job: "sample_ingest".to_string(),
                    },
                    Call::Finish {
                        run_id: RUN_ID,
                        result: Ok(json!({ "rows": 3 })),
                    },
                ],
            ),
        );
    }

    #[tokio::test]
    async fn records_the_error_when_the_job_fails() {
        let log = Arc::new(FakeIngestRunLog::default());

        let result = run_with_ingest_run_log(
            log.as_ref(),
            "sample_ingest",
            Duration::from_secs(1),
            async { Err::<IngestStats, _>("operation failed".to_string()) },
        )
        .await;

        assert_eq!(
            (result, log.calls.lock().await.clone()),
            (
                Err("operation failed".to_string()),
                vec![
                    Call::Start {
                        job: "sample_ingest".to_string(),
                    },
                    Call::Finish {
                        run_id: RUN_ID,
                        result: Err("operation failed".to_string()),
                    },
                ],
            ),
        );
    }

    #[tokio::test]
    async fn records_an_error_when_the_job_times_out() {
        let log = Arc::new(FakeIngestRunLog::default());

        let result = run_with_ingest_run_log(
            log.as_ref(),
            "sample_ingest",
            Duration::from_millis(1),
            pending::<Result<IngestStats, String>>(),
        )
        .await;

        assert_eq!(
            (result, log.calls.lock().await.clone()),
            (
                Err("sample_ingest timed out after 1ms".to_string()),
                vec![
                    Call::Start {
                        job: "sample_ingest".to_string(),
                    },
                    Call::Finish {
                        run_id: RUN_ID,
                        result: Err("sample_ingest timed out after 1ms".to_string()),
                    },
                ],
            ),
        );
    }
}
