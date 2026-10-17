use std::{future::Future, time::Duration};

use chrono::{DateTime, FixedOffset, Utc};
use core_application::ingest_run_log::IngestRunLog;

use crate::{
    jobs::{
        JobFuture, SchedulerJob,
        alpha_vantage::AlphaVantageCalendarIngest,
        boj_calendar::BojCalendarEventIngest,
        daily_bars::DailyBarsIngest,
        e_stat_calendar::EStatCalendarIngest,
        earnings_schedule::EarningsScheduleIngest,
        ecb_calendar::EcbCalendarEventIngest,
        edinet_holdings::ShareholdingStructureIngest,
        eoddata_kospi::EodDataKospiIngest,
        equity_master::EquityMasterIngest,
        fed_calendar::FedCalendarEventIngest,
        financial_summary::FinancialSummaryIngest,
        fred::FredIngest,
        fred_release_dates::FredReleaseDatesIngest,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        news::NewsAggregation,
        news_content::NewsContentFetch,
        paper_order_filling::PaperOrderFilling,
        prediction::PredictionGrading,
        strategy_task_reconcile::StrategyTaskReconcile,
        twse::TwseIndexIngest,
        us_stock_bars::{
            UsStockBarsIngest, UsStockHourlyBarsIngest, UsStockMinuteBarsDownsample,
            UsStockMinuteBarsIngest,
        },
        us_stock_master::UsStockMasterIngest,
        valuation::ValuationIngest,
    },
    state::{SchedulerDependencies, SchedulerState},
};

type JobRunner = fn(SchedulerState) -> JobFuture;

struct SupportedJob {
    identifier: &'static str,
    run: JobRunner,
    recovery_timeout: Option<Duration>,
}

const SUPPORTED_JOBS: [SupportedJob; 28] = [
    supported_ingest_job::<AlphaVantageCalendarIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<BojCalendarEventIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<DailyBarsIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<EStatCalendarIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<EarningsScheduleIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<EcbCalendarEventIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<ShareholdingStructureIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<EodDataKospiIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<EquityMasterIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<FedCalendarEventIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<FinancialSummaryIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<FredIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<FredReleaseDatesIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<MarginIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<NewsAggregation>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<NewsContentFetch>(crate::jobs::NEWS_CONTENT_JOB_TIMEOUT),
    supported_ingest_job::<PaperOrderFilling>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<PredictionGrading>(crate::jobs::WEEKLY_TIMEOUT),
    supported_ingest_job::<ShortRatioIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<ShortSaleReportIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_job::<StrategyTaskReconcile>(),
    supported_ingest_job::<TwseIndexIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<UsStockBarsIngest>(crate::jobs::US_STOCK_BARS_TIMEOUT),
    supported_ingest_job::<UsStockHourlyBarsIngest>(crate::jobs::US_STOCK_BARS_TIMEOUT),
    supported_ingest_job::<UsStockMinuteBarsIngest>(crate::jobs::US_STOCK_BARS_TIMEOUT),
    supported_ingest_job::<UsStockMinuteBarsDownsample>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<UsStockMasterIngest>(crate::jobs::DAILY_TIMEOUT),
    supported_ingest_job::<ValuationIngest>(crate::jobs::DAILY_TIMEOUT),
];

const fn supported_job<T: SchedulerJob>() -> SupportedJob {
    SupportedJob {
        identifier: T::IDENTIFIER,
        run: T::run,
        recovery_timeout: None,
    }
}

const fn supported_ingest_job<T: SchedulerJob>(timeout: Duration) -> SupportedJob {
    SupportedJob {
        identifier: T::IDENTIFIER,
        run: T::run,
        recovery_timeout: Some(timeout),
    }
}

pub fn is_supported_job(job_name: &str) -> bool {
    SUPPORTED_JOBS.iter().any(|job| job.identifier == job_name)
}

pub async fn run_job(job_name: &str, dependencies: SchedulerDependencies) -> Result<(), String> {
    let Some(job) = SUPPORTED_JOBS.iter().find(|job| job.identifier == job_name) else {
        return Err(format!("unsupported scheduled job: {job_name}"));
    };

    let state = SchedulerState { dependencies };
    let ingest_run_log = state.dependencies.ingest_run_log.clone();
    run_registered_job(
        job,
        ingest_run_log.as_ref(),
        Utc::now().fixed_offset(),
        move || (job.run)(state),
    )
    .await
}

async fn run_registered_job<F, Fut>(
    job: &SupportedJob,
    log: &dyn IngestRunLog,
    now: DateTime<FixedOffset>,
    run: F,
) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let Some(timeout) = job.recovery_timeout else {
        return run().await;
    };

    run_after_recovery(log, job.identifier, timeout, now, run).await
}

async fn run_after_recovery<F, Fut>(
    log: &dyn IngestRunLog,
    job_name: &str,
    timeout: Duration,
    now: DateTime<FixedOffset>,
    run: F,
) -> Result<(), String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let recovered = recover_interrupted_run(log, job_name, timeout, now).await?;
    if recovered > 0 {
        tracing::info!(
            task = job_name,
            recovered,
            "interrupted ingest runs recovered"
        );
    }

    run().await
}

async fn recover_interrupted_run(
    log: &dyn IngestRunLog,
    job: &str,
    timeout: Duration,
    now: DateTime<FixedOffset>,
) -> Result<u64, String> {
    let cutoff = crate::jobs::run_started_before(now, timeout)?;
    log.fail_interrupted_before(job, cutoff)
        .await
        .map_err(|error| format!("failed to recover interrupted ingest runs: {error}"))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use chrono::{DateTime, Duration as ChronoDuration, FixedOffset};
    use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
    use rstest::rstest;
    use serde_json::Value;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::{SUPPORTED_JOBS, is_supported_job, recover_interrupted_run, run_registered_job};

    #[test]
    fn supported_job_table_is_unique_and_queryable() {
        let identifiers = SUPPORTED_JOBS
            .iter()
            .map(|job| job.identifier)
            .collect::<Vec<_>>();

        assert_eq!(
            (
                identifiers
                    .iter()
                    .map(|name| is_supported_job(name))
                    .collect::<Vec<_>>(),
                identifiers.iter().collect::<HashSet<_>>().len(),
            ),
            (vec![true; SUPPORTED_JOBS.len()], SUPPORTED_JOBS.len()),
        );
    }

    #[test]
    fn only_strategy_task_reconcile_skips_interrupted_ingest_recovery() {
        assert_eq!(
            SUPPORTED_JOBS
                .iter()
                .filter(|job| job.recovery_timeout.is_none())
                .map(|job| job.identifier)
                .collect::<Vec<_>>(),
            vec!["strategy_task_reconcile"],
        );
    }

    #[rstest]
    #[case::us_stock_minute_bars_ingest("us_stock_minute_bars_ingest", true)]
    #[case::us_stock_hourly_bars_ingest("us_stock_hourly_bars_ingest", true)]
    #[case::us_stock_minute_bars_downsample("us_stock_minute_bars_downsample", true)]
    #[case::event_dispatch("event_dispatch", false)]
    #[case::unknown("unknown_job", false)]
    fn rejects_jobs_outside_the_one_shot_contract(#[case] job_name: &str, #[case] expected: bool) {
        assert_eq!(is_supported_job(job_name), expected);
    }

    #[derive(Default)]
    struct FakeIngestRunLog {
        calls: Mutex<Vec<(String, DateTime<FixedOffset>)>>,
        fail_recovery: bool,
    }

    #[async_trait]
    impl IngestRunLog for FakeIngestRunLog {
        async fn start(&self, _job: &str) -> Result<Uuid, PersistenceError> {
            Ok(Uuid::from_u128(1))
        }

        async fn probe_commit(&self, _run_id: Uuid) -> Result<Duration, PersistenceError> {
            Ok(Duration::ZERO)
        }

        async fn finish(
            &self,
            _run_id: Uuid,
            _result: Result<Value, String>,
        ) -> Result<(), PersistenceError> {
            Ok(())
        }

        async fn fail_interrupted_before(
            &self,
            job: &str,
            started_before: DateTime<FixedOffset>,
        ) -> Result<u64, PersistenceError> {
            self.calls
                .lock()
                .await
                .push((job.to_owned(), started_before));
            if self.fail_recovery {
                Err(PersistenceError::Database(
                    "database unavailable".to_string(),
                ))
            } else {
                Ok(1)
            }
        }
    }

    #[tokio::test]
    async fn recovers_only_runs_older_than_the_job_timeout() {
        let now = DateTime::parse_from_rfc3339("2039-05-10T16:00:00+00:00")
            .expect("valid fixed timestamp");
        let log = Arc::new(FakeIngestRunLog::default());

        let result = recover_interrupted_run(
            log.as_ref(),
            "sample_ingest",
            Duration::from_secs(2 * 60 * 60),
            now,
        )
        .await;

        assert_eq!(
            (result, log.calls.lock().await.clone()),
            (
                Ok(1),
                vec![("sample_ingest".to_owned(), now - ChronoDuration::hours(2))],
            ),
        );
    }

    #[tokio::test]
    async fn registered_ingest_job_recovers_before_running_and_stops_on_recovery_failure() {
        let now = DateTime::parse_from_rfc3339("2039-05-10T16:00:00+00:00")
            .expect("valid fixed timestamp");
        let job = SUPPORTED_JOBS
            .iter()
            .find(|job| job.identifier == "daily_bars_ingest")
            .expect("daily bars job is supported");
        let log = Arc::new(FakeIngestRunLog {
            fail_recovery: true,
            ..Default::default()
        });
        let job_started = Arc::new(AtomicBool::new(false));
        let job_started_in_future = Arc::clone(&job_started);

        let result = run_registered_job(job, log.as_ref(), now, move || async move {
            job_started_in_future.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;

        assert_eq!(
            (
                result,
                log.calls.lock().await.clone(),
                job_started.load(Ordering::SeqCst),
            ),
            (
                Err("failed to recover interrupted ingest runs: database unavailable".to_string()),
                vec![(
                    "daily_bars_ingest".to_owned(),
                    now - ChronoDuration::hours(2),
                )],
                false,
            ),
        );
    }
}
