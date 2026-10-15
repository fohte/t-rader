use std::time::Duration;

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
        event_dispatch::EventDispatch,
        fed_calendar::FedCalendarEventIngest,
        financial_summary::FinancialSummaryIngest,
        fred::FredIngest,
        fred_release_dates::FredReleaseDatesIngest,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        news::NewsAggregation,
        news_content::NewsContentFetch,
        paper_order_filling::PaperOrderFilling,
        prediction::PredictionGrading,
        run_started_before,
        strategy_task_reconcile::StrategyTaskReconcile,
        twse::TwseIndexIngest,
        us_stock_bars::UsStockBarsIngest,
        us_stock_master::UsStockMasterIngest,
        valuation::ValuationIngest,
    },
    scheduler::RECOVERABLE_INGEST_JOBS,
    state::{SchedulerDependencies, SchedulerState},
};

type JobRunner = fn(SchedulerState) -> JobFuture;

struct SupportedJob {
    identifier: &'static str,
    run: JobRunner,
}

const SUPPORTED_JOBS: [SupportedJob; 26] = [
    supported_job::<AlphaVantageCalendarIngest>(),
    supported_job::<BojCalendarEventIngest>(),
    supported_job::<DailyBarsIngest>(),
    supported_job::<EStatCalendarIngest>(),
    supported_job::<EarningsScheduleIngest>(),
    supported_job::<EcbCalendarEventIngest>(),
    supported_job::<ShareholdingStructureIngest>(),
    supported_job::<EodDataKospiIngest>(),
    supported_job::<EquityMasterIngest>(),
    supported_job::<EventDispatch>(),
    supported_job::<FedCalendarEventIngest>(),
    supported_job::<FinancialSummaryIngest>(),
    supported_job::<FredIngest>(),
    supported_job::<FredReleaseDatesIngest>(),
    supported_job::<MarginIngest>(),
    supported_job::<NewsAggregation>(),
    supported_job::<NewsContentFetch>(),
    supported_job::<PaperOrderFilling>(),
    supported_job::<PredictionGrading>(),
    supported_job::<ShortRatioIngest>(),
    supported_job::<ShortSaleReportIngest>(),
    supported_job::<StrategyTaskReconcile>(),
    supported_job::<TwseIndexIngest>(),
    supported_job::<UsStockBarsIngest>(),
    supported_job::<UsStockMasterIngest>(),
    supported_job::<ValuationIngest>(),
];

const fn supported_job<T: SchedulerJob>() -> SupportedJob {
    SupportedJob {
        identifier: T::IDENTIFIER,
        run: T::run,
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
    if let Some((_, timeout)) = RECOVERABLE_INGEST_JOBS
        .iter()
        .find(|(identifier, _)| *identifier == job_name)
    {
        let recovered = recover_interrupted_run(
            state.dependencies.ingest_run_log.as_ref(),
            job_name,
            *timeout,
            Utc::now().fixed_offset(),
        )
        .await?;
        if recovered > 0 {
            tracing::info!(
                task = job_name,
                recovered,
                "interrupted ingest runs recovered"
            );
        }
    }

    (job.run)(state).await
}

async fn recover_interrupted_run(
    log: &dyn IngestRunLog,
    job_name: &str,
    timeout: Duration,
    now: DateTime<FixedOffset>,
) -> Result<u64, String> {
    let started_before = run_started_before(now, timeout)?;
    log.fail_interrupted_before(job_name, started_before)
        .await
        .map_err(|error| format!("failed to recover interrupted ingest runs: {error}"))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc, time::Duration};

    use async_trait::async_trait;
    use chrono::{DateTime, Duration as ChronoDuration, FixedOffset};
    use core_application::{ingest_run_log::IngestRunLog, persistence::PersistenceError};
    use rstest::rstest;
    use serde_json::Value;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use super::{SUPPORTED_JOBS, is_supported_job, recover_interrupted_run};

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

    #[rstest]
    #[case::trigger_evaluation("trigger_evaluation", false)]
    #[case::ingest_run_recovery("ingest_run_recovery", false)]
    #[case::unknown("unknown_job", false)]
    fn rejects_jobs_outside_the_one_shot_contract(#[case] job_name: &str, #[case] expected: bool) {
        assert_eq!(is_supported_job(job_name), expected);
    }

    #[derive(Default)]
    struct FakeIngestRunLog {
        calls: Mutex<Vec<(String, DateTime<FixedOffset>)>>,
    }

    #[async_trait]
    impl IngestRunLog for FakeIngestRunLog {
        async fn start(&self, _job: &str) -> Result<Uuid, PersistenceError> {
            Ok(Uuid::from_u128(1))
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
            Ok(1)
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
}
