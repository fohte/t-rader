use std::time::Duration;
use std::{future::Future, pin::Pin};

use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use chrono_tz::Asia::Tokyo;
use core_application::bars::{
    US_STOCK_DAILY_BARS_MAX_RUN_DURATION, US_STOCK_INTRADAY_BARS_MAX_RUN_DURATION,
};
use core_application::ingest_run_log::IngestRunLog;
use core_application::screener::ScreenerSnapshotUseCases;
use serde::Serialize;
use tokio::time::timeout;
use uuid::Uuid;

use crate::state::SchedulerState;

pub mod alpha_vantage;
pub mod boj_calendar;
mod calendar_event;
pub mod daily_bars;
pub mod e_stat_calendar;
pub mod earnings_schedule;
pub mod ecb_calendar;
pub mod edinet_holdings;
pub mod eoddata_kospi;
pub mod equity_master;
pub mod fed_calendar;
pub mod financial_summary;
pub mod fred;
pub mod fred_release_dates;
pub mod jquants;
pub mod news;
pub mod news_content;
pub mod paper_order_filling;
pub mod prediction;
pub mod strategy_task_reconcile;
pub mod twse;
pub mod us_stock_bars;
pub mod us_stock_master;
pub mod us_stock_sip_bar_refetch;
pub mod valuation;

pub(super) const DAILY_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
pub(super) const WEEKLY_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);
pub(super) const US_STOCK_BARS_TIMEOUT: Duration = DAILY_TIMEOUT;
pub(crate) const NEWS_CONTENT_JOB_TIMEOUT: Duration = Duration::from_secs(10 * 60);

// 分足・時間足 CronJob の activeDeadlineSeconds: 3300 は
// fohte/infra/kubernetes/home/manifests/t-rader/values.yaml に定義。
const US_STOCK_BARS_CRONJOB_DEADLINE: Duration = Duration::from_secs(3_300);

const _: () = assert!(
    US_STOCK_DAILY_BARS_MAX_RUN_DURATION.as_secs() < US_STOCK_BARS_TIMEOUT.as_secs(),
    "US stock daily bars budget must be shorter than the scheduler timeout",
);
const _: () = assert!(
    US_STOCK_INTRADAY_BARS_MAX_RUN_DURATION.as_secs() < US_STOCK_BARS_CRONJOB_DEADLINE.as_secs(),
    "US stock intraday bars budget must be shorter than the CronJob deadline",
);

pub(crate) type JobFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;

pub(crate) trait SchedulerJob {
    const IDENTIFIER: &'static str;

    fn run(state: SchedulerState) -> JobFuture;
}

pub(super) fn today_in_tokyo(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&Tokyo).date_naive()
}

pub(super) fn require_source<T>(source: Option<T>, name: &str) -> Result<T, String> {
    source.ok_or_else(|| format!("{name} is not configured"))
}

pub(super) async fn refresh_screener_snapshot(
    use_cases: &ScreenerSnapshotUseCases,
) -> Result<(), String> {
    use_cases
        .refresh_snapshot()
        .await
        .map_err(|error| format!("failed to refresh screener snapshot: {error}"))
}

pub(super) async fn run_and_refresh_screener_snapshot<T, Fut>(
    use_cases: &ScreenerSnapshotUseCases,
    ingest: Fut,
) -> Result<T, String>
where
    Fut: Future<Output = Result<T, String>>,
{
    let stats = ingest.await?;
    refresh_screener_snapshot(use_cases).await?;
    Ok(stats)
}

pub(super) async fn run_with_state<F, Fut>(
    state: SchedulerState,
    task_name: &'static str,
    timeout: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    match run_with_timeout(task_name, timeout, task(state)).await {
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
    state: SchedulerState,
    task_name: &'static str,
    timeout: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<Stats, String>>,
    Stats: Serialize + Send + 'static,
{
    let ingest_run_log = state.dependencies.ingest_run_log.clone();
    run_with_ingest_run_log(ingest_run_log.as_ref(), task_name, timeout, task(state)).await
}

pub(super) async fn run_with_ingest_run_log_state_with_run_id_and_failure_stats<F, Fut, Stats>(
    state: SchedulerState,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState, Uuid) -> Fut,
    Fut: Future<Output = Result<Stats, (Box<Stats>, String)>>,
    Stats: Serialize + Send + 'static,
{
    let ingest_run_log = state.dependencies.ingest_run_log.clone();
    run_with_ingest_run_log_inner(
        ingest_run_log.as_ref(),
        task_name,
        duration,
        move |run_id| async move {
            task(state, run_id)
                .await
                .map_err(|(stats, error)| IngestTaskFailure::ErrorWithStats {
                    stats: *stats,
                    error,
                })
        },
    )
    .await
}

pub(super) async fn run_with_ingest_run_log_state_and_failure_stats<F, Fut, Stats>(
    state: SchedulerState,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(SchedulerState) -> Fut,
    Fut: Future<Output = Result<Stats, (Stats, String)>>,
    Stats: Serialize + Send + 'static,
{
    let ingest_run_log = state.dependencies.ingest_run_log.clone();
    run_with_ingest_run_log_with_failure_stats(
        ingest_run_log.as_ref(),
        task_name,
        duration,
        task(state),
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
    Stats: Serialize + Send + 'static,
{
    run_with_ingest_run_log_inner(ingest_run_log, task_name, duration, move |_| async move {
        task.await.map_err(IngestTaskFailure::Error)
    })
    .await
}

async fn run_with_ingest_run_log_with_failure_stats<F, Stats>(
    ingest_run_log: &dyn IngestRunLog,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: Future<Output = Result<Stats, (Stats, String)>>,
    Stats: Serialize + Send + 'static,
{
    run_with_ingest_run_log_inner(ingest_run_log, task_name, duration, move |_| async move {
        task.await
            .map_err(|(stats, error)| IngestTaskFailure::ErrorWithStats { stats, error })
    })
    .await
}

enum IngestTaskFailure<Stats> {
    Error(String),
    ErrorWithStats { stats: Stats, error: String },
}

async fn run_with_ingest_run_log_inner<F, Fut, Stats>(
    ingest_run_log: &dyn IngestRunLog,
    task_name: &'static str,
    duration: Duration,
    task: F,
) -> Result<(), String>
where
    F: FnOnce(Uuid) -> Fut,
    Fut: Future<Output = Result<Stats, IngestTaskFailure<Stats>>>,
    Stats: Serialize + Send + 'static,
{
    let run_id = match ingest_run_log.start(task_name).await {
        Ok(run_id) => run_id,
        Err(error) => {
            let error = format!("failed to start ingest run log: {error}");
            tracing::error!(task = task_name, %error, "failed to start scheduled job");
            return Err(error);
        }
    };

    let mut completed_stats = None;
    let (recorded_result, failure_stats, timed_out) = match timeout(duration, task(run_id)).await {
        Ok(Ok(stats)) => {
            let recorded_result = serde_json::to_value(&stats)
                .map_err(|error| format!("failed to serialize job stats: {error}"));
            completed_stats = Some(stats);
            (recorded_result, None, false)
        }
        Ok(Err(IngestTaskFailure::Error(error))) => (Err(error), None, false),
        Ok(Err(IngestTaskFailure::ErrorWithStats { stats, error })) => {
            let recorded = match serde_json::to_value(&stats) {
                Ok(stats) => (Err(error), Some(stats), false),
                Err(serialization_error) => (
                    Err(format!(
                        "{error}; failed to serialize job stats: {serialization_error}"
                    )),
                    None,
                    false,
                ),
            };
            completed_stats = Some(stats);
            recorded
        }
        Err(_) => {
            let error = format!("{task_name} timed out after {duration:?}");
            tracing::error!(task = task_name, ?duration, "scheduled job timed out");
            (Err(error), None, true)
        }
    };

    let finish_result = match (&recorded_result, failure_stats) {
        (Err(error), Some(stats)) => {
            ingest_run_log
                .finish_with_stats_and_error(run_id, stats, error.clone())
                .await
        }
        _ => ingest_run_log.finish(run_id, recorded_result.clone()).await,
    };
    drop(completed_stats);
    if let Err(log_error) = finish_result {
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

pub(crate) fn run_started_before(
    now: DateTime<chrono::FixedOffset>,
    duration: Duration,
) -> Result<DateTime<chrono::FixedOffset>, String> {
    let duration = ChronoDuration::from_std(duration)
        .map_err(|error| format!("job timeout cannot be represented: {error}"))?;
    Ok(now - duration)
}

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use chrono::{NaiveDate, TimeZone, Utc};
    use core_application::{
        ingest_run_log::IngestRunLog,
        persistence::PersistenceError,
        screener::{ScreenerSnapshotRefresher, ScreenerSnapshotUseCases},
    };
    use serde::Serialize;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use rstest::rstest;

    use super::{
        TimedTaskResult, run_and_refresh_screener_snapshot, run_started_before,
        run_with_ingest_run_log, run_with_ingest_run_log_with_failure_stats, run_with_timeout,
        today_in_tokyo,
    };

    const RUN_ID: Uuid = Uuid::from_u128(1);

    struct FakeScreenerSnapshotRefresher {
        calls: Arc<AtomicUsize>,
        fail: bool,
    }

    #[async_trait]
    impl ScreenerSnapshotRefresher for FakeScreenerSnapshotRefresher {
        async fn refresh_snapshot(&self) -> Result<(), PersistenceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(PersistenceError::Database("refresh failed".to_owned()))
            } else {
                Ok(())
            }
        }
    }

    #[rstest]
    #[case::refreshes_after_success(Ok(5), false, Ok(5), 1)]
    #[case::skips_refresh_after_ingest_failure(Err("ingest failed".to_owned()), false, Err("ingest failed".to_owned()), 0)]
    #[case::returns_refresh_failure(Ok(5), true, Err("failed to refresh screener snapshot: refresh failed".to_owned()), 1)]
    #[tokio::test]
    async fn refreshes_screener_snapshot_after_successful_ingestion(
        #[case] ingest_result: Result<usize, String>,
        #[case] refresh_fails: bool,
        #[case] expected_result: Result<usize, String>,
        #[case] expected_refresh_count: usize,
    ) {
        let calls = Arc::new(AtomicUsize::new(0));
        let use_cases = ScreenerSnapshotUseCases::new(Arc::new(FakeScreenerSnapshotRefresher {
            calls: calls.clone(),
            fail: refresh_fails,
        }));

        let result = run_and_refresh_screener_snapshot(&use_cases, async { ingest_result }).await;

        assert_eq!(
            (result, calls.load(Ordering::SeqCst)),
            (expected_result, expected_refresh_count),
        );
    }

    #[test]
    fn today_in_tokyo_uses_the_tokyo_calendar_date() {
        let now = Utc
            .with_ymd_and_hms(2039, 5, 10, 16, 0, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(
            today_in_tokyo(now),
            NaiveDate::from_ymd_opt(2039, 5, 11).expect("valid date"),
        );
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Start {
            job: String,
        },
        Finish {
            run_id: Uuid,
            result: Result<Value, String>,
        },
        FinishWithStats {
            run_id: Uuid,
            stats: Value,
            error: String,
        },
    }

    #[derive(Default)]
    struct FakeIngestRunLog {
        calls: Mutex<Vec<Call>>,
        fail_recovery: bool,
        fail_start: bool,
        fail_finish: bool,
        guard_observation: Option<Arc<GuardObservation>>,
    }

    #[derive(Default)]
    struct GuardObservation {
        dropped: AtomicBool,
        dropped_before_finish: AtomicBool,
    }

    impl FakeIngestRunLog {
        fn observe_finish(&self) {
            if let Some(observation) = &self.guard_observation {
                observation
                    .dropped_before_finish
                    .store(observation.dropped.load(Ordering::SeqCst), Ordering::SeqCst);
            }
        }
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

        async fn probe_commit(&self, _run_id: Uuid) -> Result<Duration, PersistenceError> {
            Ok(Duration::ZERO)
        }

        async fn finish(
            &self,
            run_id: Uuid,
            result: Result<Value, String>,
        ) -> Result<(), PersistenceError> {
            self.observe_finish();
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

        async fn finish_with_stats_and_error(
            &self,
            run_id: Uuid,
            stats: Value,
            error: String,
        ) -> Result<(), PersistenceError> {
            self.observe_finish();
            self.calls.lock().await.push(Call::FinishWithStats {
                run_id,
                stats,
                error,
            });
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
            if self.fail_recovery {
                Err(PersistenceError::Database(
                    "database unavailable".to_string(),
                ))
            } else {
                Ok(0)
            }
        }
    }

    #[derive(Serialize)]
    struct IngestStats {
        rows: usize,
    }

    struct CompletionGuard(Arc<GuardObservation>);

    impl Drop for CompletionGuard {
        fn drop(&mut self) {
            self.0.dropped.store(true, Ordering::SeqCst);
        }
    }

    #[derive(Serialize)]
    struct IngestStatsWithCompletionGuard {
        rows: usize,
        #[serde(skip)]
        _guard: CompletionGuard,
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
    async fn keeps_completion_guard_until_stats_are_persisted() {
        let observation = Arc::new(GuardObservation::default());
        let log = Arc::new(FakeIngestRunLog {
            guard_observation: Some(observation.clone()),
            ..FakeIngestRunLog::default()
        });

        let result = run_with_ingest_run_log(
            log.as_ref(),
            "sample_ingest",
            Duration::from_secs(1),
            async {
                Ok(IngestStatsWithCompletionGuard {
                    rows: 3,
                    _guard: CompletionGuard(observation.clone()),
                })
            },
        )
        .await;

        assert_eq!(
            (
                result,
                observation.dropped_before_finish.load(Ordering::SeqCst),
                observation.dropped.load(Ordering::SeqCst),
                log.calls.lock().await.clone(),
            ),
            (
                Ok(()),
                false,
                true,
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
    async fn keeps_failure_guard_until_partial_stats_are_persisted() {
        let observation = Arc::new(GuardObservation::default());
        let log = Arc::new(FakeIngestRunLog {
            guard_observation: Some(observation.clone()),
            ..FakeIngestRunLog::default()
        });

        let result = run_with_ingest_run_log_with_failure_stats(
            log.as_ref(),
            "sample_ingest",
            Duration::from_secs(1),
            async {
                Err((
                    IngestStatsWithCompletionGuard {
                        rows: 3,
                        _guard: CompletionGuard(observation.clone()),
                    },
                    "operation failed".to_owned(),
                ))
            },
        )
        .await;

        assert_eq!(
            (
                result,
                observation.dropped_before_finish.load(Ordering::SeqCst),
                observation.dropped.load(Ordering::SeqCst),
                log.calls.lock().await.clone(),
            ),
            (
                Err("operation failed".to_owned()),
                false,
                true,
                vec![
                    Call::Start {
                        job: "sample_ingest".to_owned(),
                    },
                    Call::FinishWithStats {
                        run_id: RUN_ID,
                        stats: json!({ "rows": 3 }),
                        error: "operation failed".to_owned(),
                    },
                ],
            ),
        );
    }

    #[test]
    fn computes_the_recovery_cutoff_from_the_job_timeout() {
        let now = Utc
            .with_ymd_and_hms(2039, 5, 10, 16, 0, 0)
            .single()
            .expect("valid timestamp");

        assert_eq!(
            run_started_before(now.fixed_offset(), Duration::from_secs(3600)),
            Ok((now - chrono::Duration::hours(1)).fixed_offset()),
        );
    }

    #[tokio::test]
    async fn records_partial_stats_when_the_job_fails_with_stats() {
        let log = Arc::new(FakeIngestRunLog::default());

        let result = run_with_ingest_run_log_with_failure_stats(
            log.as_ref(),
            "news_content_fetch",
            Duration::from_secs(1),
            async { Err((IngestStats { rows: 3 }, "firecrawl_402".to_owned())) },
        )
        .await;

        assert_eq!(
            (result, log.calls.lock().await.clone()),
            (
                Err("firecrawl_402".to_owned()),
                vec![
                    Call::Start {
                        job: "news_content_fetch".to_owned(),
                    },
                    Call::FinishWithStats {
                        run_id: RUN_ID,
                        stats: json!({ "rows": 3 }),
                        error: "firecrawl_402".to_owned(),
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
                },],
                false,
            ),
        );
    }

    #[tokio::test]
    async fn does_not_recover_worker_runs_before_starting_the_job() {
        let log = Arc::new(FakeIngestRunLog {
            fail_recovery: true,
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
                true,
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
