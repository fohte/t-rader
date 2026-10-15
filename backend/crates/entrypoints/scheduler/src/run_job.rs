use std::{future::Future, pin::Pin};

use graphile_worker::{
    Job, SharedTaskDetails, TaskHandler, WorkerContext, run_task_from_worker_ctx,
};
use graphile_worker_extensions::{Extensions, ReadOnlyExtensions};
use serde_json::Value;
use sqlx::PgPool;

use crate::{
    jobs::{
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
        strategy_task_reconcile::StrategyTaskReconcile,
        twse::TwseIndexIngest,
        us_stock_bars::UsStockBarsIngest,
        us_stock_master::UsStockMasterIngest,
        valuation::ValuationIngest,
    },
    scheduler::GRAPHILE_WORKER_SCHEMA,
    state::{SchedulerDependencies, SchedulerState},
};

type JobRunner =
    fn(WorkerContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;

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

const fn supported_job<T: TaskHandler>() -> SupportedJob {
    SupportedJob {
        identifier: T::IDENTIFIER,
        run: run_task::<T>,
    }
}

fn run_task<T: TaskHandler>(
    context: WorkerContext,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>> {
    Box::pin(run_task_from_worker_ctx::<T>(context))
}

pub fn is_supported_job(job_name: &str) -> bool {
    SUPPORTED_JOBS.iter().any(|job| job.identifier == job_name)
}

pub async fn run_job(
    job_name: &str,
    pool: PgPool,
    dependencies: SchedulerDependencies,
) -> Result<(), String> {
    let Some(job) = SUPPORTED_JOBS.iter().find(|job| job.identifier == job_name) else {
        return Err(format!("unsupported scheduled job: {job_name}"));
    };

    let context = build_worker_context(job_name, pool, dependencies);
    (job.run)(context).await
}

fn build_worker_context(
    job_name: &str,
    pool: PgPool,
    dependencies: SchedulerDependencies,
) -> WorkerContext {
    let mut extensions = Extensions::new();
    let _ = extensions.insert(SchedulerState { dependencies });

    WorkerContext::builder()
        .payload(Value::Null)
        .pg_pool(pool)
        .schema(GRAPHILE_WORKER_SCHEMA)
        .job(Job::builder().task_identifier(job_name.to_owned()).build())
        .worker_id("run-job")
        .extensions(ReadOnlyExtensions::new(extensions))
        .task_details(SharedTaskDetails::default())
        .use_local_time(false)
        .build()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rstest::rstest;

    use super::{SUPPORTED_JOBS, is_supported_job};

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
}
