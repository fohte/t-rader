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

const SUPPORTED_JOB_IDENTIFIERS: [&str; 26] = [
    AlphaVantageCalendarIngest::IDENTIFIER,
    BojCalendarEventIngest::IDENTIFIER,
    DailyBarsIngest::IDENTIFIER,
    EStatCalendarIngest::IDENTIFIER,
    EarningsScheduleIngest::IDENTIFIER,
    EcbCalendarEventIngest::IDENTIFIER,
    ShareholdingStructureIngest::IDENTIFIER,
    EodDataKospiIngest::IDENTIFIER,
    EquityMasterIngest::IDENTIFIER,
    EventDispatch::IDENTIFIER,
    FedCalendarEventIngest::IDENTIFIER,
    FinancialSummaryIngest::IDENTIFIER,
    FredIngest::IDENTIFIER,
    FredReleaseDatesIngest::IDENTIFIER,
    MarginIngest::IDENTIFIER,
    NewsAggregation::IDENTIFIER,
    NewsContentFetch::IDENTIFIER,
    PaperOrderFilling::IDENTIFIER,
    PredictionGrading::IDENTIFIER,
    ShortRatioIngest::IDENTIFIER,
    ShortSaleReportIngest::IDENTIFIER,
    StrategyTaskReconcile::IDENTIFIER,
    TwseIndexIngest::IDENTIFIER,
    UsStockBarsIngest::IDENTIFIER,
    UsStockMasterIngest::IDENTIFIER,
    ValuationIngest::IDENTIFIER,
];

pub fn is_supported_job(job_name: &str) -> bool {
    SUPPORTED_JOB_IDENTIFIERS.contains(&job_name)
}

pub async fn run_job(
    job_name: &str,
    pool: PgPool,
    dependencies: SchedulerDependencies,
) -> Result<(), String> {
    if !is_supported_job(job_name) {
        return Err(format!("unsupported scheduled job: {job_name}"));
    }

    let context = build_worker_context(job_name, pool, dependencies);
    match job_name {
        AlphaVantageCalendarIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<AlphaVantageCalendarIngest>(context).await
        }
        BojCalendarEventIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<BojCalendarEventIngest>(context).await
        }
        DailyBarsIngest::IDENTIFIER => run_task_from_worker_ctx::<DailyBarsIngest>(context).await,
        EStatCalendarIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<EStatCalendarIngest>(context).await
        }
        EarningsScheduleIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<EarningsScheduleIngest>(context).await
        }
        EcbCalendarEventIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<EcbCalendarEventIngest>(context).await
        }
        ShareholdingStructureIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<ShareholdingStructureIngest>(context).await
        }
        EodDataKospiIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<EodDataKospiIngest>(context).await
        }
        EquityMasterIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<EquityMasterIngest>(context).await
        }
        EventDispatch::IDENTIFIER => run_task_from_worker_ctx::<EventDispatch>(context).await,
        FedCalendarEventIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<FedCalendarEventIngest>(context).await
        }
        FinancialSummaryIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<FinancialSummaryIngest>(context).await
        }
        FredIngest::IDENTIFIER => run_task_from_worker_ctx::<FredIngest>(context).await,
        FredReleaseDatesIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<FredReleaseDatesIngest>(context).await
        }
        MarginIngest::IDENTIFIER => run_task_from_worker_ctx::<MarginIngest>(context).await,
        NewsAggregation::IDENTIFIER => run_task_from_worker_ctx::<NewsAggregation>(context).await,
        NewsContentFetch::IDENTIFIER => run_task_from_worker_ctx::<NewsContentFetch>(context).await,
        PaperOrderFilling::IDENTIFIER => {
            run_task_from_worker_ctx::<PaperOrderFilling>(context).await
        }
        PredictionGrading::IDENTIFIER => {
            run_task_from_worker_ctx::<PredictionGrading>(context).await
        }
        ShortRatioIngest::IDENTIFIER => run_task_from_worker_ctx::<ShortRatioIngest>(context).await,
        ShortSaleReportIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<ShortSaleReportIngest>(context).await
        }
        StrategyTaskReconcile::IDENTIFIER => {
            run_task_from_worker_ctx::<StrategyTaskReconcile>(context).await
        }
        TwseIndexIngest::IDENTIFIER => run_task_from_worker_ctx::<TwseIndexIngest>(context).await,
        UsStockBarsIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<UsStockBarsIngest>(context).await
        }
        UsStockMasterIngest::IDENTIFIER => {
            run_task_from_worker_ctx::<UsStockMasterIngest>(context).await
        }
        ValuationIngest::IDENTIFIER => run_task_from_worker_ctx::<ValuationIngest>(context).await,
        _ => unreachable!("supported job identifier must have a handler"),
    }
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

    use super::{SUPPORTED_JOB_IDENTIFIERS, is_supported_job};

    #[test]
    fn supports_every_registered_one_shot_job() {
        assert_eq!(
            (
                SUPPORTED_JOB_IDENTIFIERS.map(is_supported_job),
                SUPPORTED_JOB_IDENTIFIERS
                    .into_iter()
                    .collect::<HashSet<_>>()
                    .len(),
            ),
            ([true; 26], 26),
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
