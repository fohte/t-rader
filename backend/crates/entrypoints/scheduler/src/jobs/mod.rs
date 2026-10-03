use std::future::Future;
use std::time::Duration;

use core_application::ingest_run_log::IngestRunLog;
use graphile_worker::WorkerContext;
use serde::Serialize;
use tokio::time::timeout;

use crate::state::SchedulerState;

pub mod daily_bars;
pub mod e_stat_calendar;
pub mod earnings_schedule;
pub mod edinet_holdings;
pub mod equity_master;
pub mod financial_summary;
pub mod fred;
pub mod ingest_run_recovery;
pub mod jquants;
pub mod news;
pub mod prediction;
pub mod strategy_task_reconcile;
pub mod trigger_evaluation;
pub mod valuation;

pub(super) const DAILY_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
pub(super) const WEEKLY_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);

pub(super) fn require_source<T>(source: Option<T>, name: &str) -> Result<T, String> {
    source.ok_or_else(|| format!("{name} is not configured"))
}

pub(super) async fn run_with_state<F, Fut>(
    context: WorkerContext,
    task_name: &'static str,
    timeout: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let Some(state) = context.get_ext::<SchedulerState>() else {
        return Err("scheduler state is not configured".to_string());
    };

    match run_with_timeout(task_name, timeout, task(state.clone())).await {
        TimedTaskResult::Completed(Ok(())) => {
            tracing::info!(task = task_name, "scheduled job completed");
            Ok(())
        }
        TimedTaskResult::Completed(Err(error)) => {
            tracing::warn!(task = task_name, %error, "scheduled job failed");
            Err(error)
        }
        TimedTaskResult::TimedOut(error) => Err(error),
    }
}

pub(super) async fn run_with_ingest_run_log_state<F, Fut, Stats>(
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

#[derive(Debug, PartialEq, Eq)]
enum TimedTaskResult<Stats> {
    Completed(Result<Stats, String>),
    TimedOut(String),
}

async fn run_with_timeout<F, Stats>(
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> TimedTaskResult<Stats>
where
    F: Future<Output = Result<Stats, String>>,
{
    match timeout(duration, task).await {
        Ok(result) => TimedTaskResult::Completed(result),
        Err(_) => {
            let error = format!("{task_name} timed out after {duration:?}");
            tracing::error!(task = task_name, ?duration, "scheduled job timed out");
            TimedTaskResult::TimedOut(error)
        }
    }
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
    let run_id = match ingest_run_log.start(task_name).await {
        Ok(run_id) => run_id,
        Err(error) => {
            let error = format!("failed to start ingest run log: {error}");
            tracing::error!(task = task_name, %error, "failed to start scheduled job");
            return Err(error);
        }
    };

    let (task_result, timed_out) = match run_with_timeout(task_name, duration, task).await {
        TimedTaskResult::Completed(result) => (result, false),
        TimedTaskResult::TimedOut(error) => (Err(error), true),
    };
    let recorded_result = task_result.and_then(|stats| {
        serde_json::to_value(stats)
            .map_err(|error| format!("failed to serialize job stats: {error}"))
    });

    if let Err(log_error) = ingest_run_log.finish(run_id, recorded_result.clone()).await {
        match &recorded_result {
            Ok(_) => {
                tracing::error!(
                    task = task_name,
                    %log_error,
                    "scheduled job completed but its ingest run result could not be persisted"
                );
                return Ok(());
            }
            Err(task_error) => {
                let error = format!("{task_error}; failed to finish ingest run log: {log_error}");
                tracing::error!(task = task_name, %error, "failed to persist scheduled job result");
                return Err(error);
            }
        }
    }

    match recorded_result {
        Ok(_) => {
            tracing::info!(task = task_name, "scheduled job completed");
            Ok(())
        }
        Err(error) => {
            if !timed_out {
                tracing::warn!(task = task_name, %error, "scheduled job failed");
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
    use serde::Serialize;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use rstest::rstest;

    use super::{TimedTaskResult, run_with_ingest_run_log, run_with_timeout};

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
        fail_start: bool,
        fail_finish: bool,
    }

    #[async_trait]
    impl IngestRunLog for FakeIngestRunLog {
        async fn start(&self, job: &str) -> Result<Uuid, PersistenceError> {
            self.calls.lock().await.push(Call::Start {
                job: job.to_string(),
            });
            if self.fail_start {
                Err(PersistenceError::Database(
                    "database unavailable".to_string(),
                ))
            } else {
                Ok(RUN_ID)
            }
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
            if self.fail_finish {
                Err(PersistenceError::Database(
                    "database unavailable".to_string(),
                ))
            } else {
                Ok(())
            }
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

    #[rstest]
    #[case::success(Ok(()), TimedTaskResult::Completed(Ok(())))]
    #[case::failure(
        Err("operation failed".to_string()),
        TimedTaskResult::Completed(Err("operation failed".to_string()))
    )]
    #[tokio::test]
    async fn returns_completed_job_results(
        #[case] task_result: Result<(), String>,
        #[case] expected: TimedTaskResult<()>,
    ) {
        assert_eq!(
            run_with_timeout("sample_task", Duration::from_secs(1), async { task_result }).await,
            expected,
        );
    }

    #[tokio::test]
    async fn marks_jobs_that_exceed_their_timeout() {
        assert_eq!(
            run_with_timeout(
                "sample_task",
                Duration::from_millis(1),
                pending::<Result<(), String>>(),
            )
            .await,
            TimedTaskResult::TimedOut("sample_task timed out after 1ms".to_string()),
        );
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
    async fn does_not_run_the_job_when_start_logging_fails() {
        let log = Arc::new(FakeIngestRunLog {
            fail_start: true,
            ..Default::default()
        });
        let task_started = Arc::new(AtomicBool::new(false));
        let task_started_in_future = Arc::clone(&task_started);

        let result = run_with_ingest_run_log(
            log.as_ref(),
            "sample_ingest",
            Duration::from_secs(1),
            async move {
                task_started_in_future.store(true, Ordering::SeqCst);
                Ok(IngestStats { rows: 3 })
            },
        )
        .await;

        assert_eq!(
            (
                result,
                log.calls.lock().await.clone(),
                task_started.load(Ordering::SeqCst),
            ),
            (
                Err("failed to start ingest run log: database unavailable".to_string()),
                vec![Call::Start {
                    job: "sample_ingest".to_string(),
                }],
                false,
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

    #[tokio::test]
    async fn does_not_retry_a_successful_job_when_finish_logging_fails() {
        let log = Arc::new(FakeIngestRunLog {
            fail_finish: true,
            ..Default::default()
        });

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
}
