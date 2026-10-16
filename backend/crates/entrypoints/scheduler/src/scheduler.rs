use std::{future::Future, time::Duration};

use core_application::ingest_status::{
    BOJ_CALENDAR_EVENT_INGEST_JOB, ECB_CALENDAR_EVENT_INGEST_JOB, FED_CALENDAR_EVENT_INGEST_JOB,
};
use graphile_worker::{Cron, CrontabFill, TaskHandler, Worker, WorkerOptions};
use sqlx::PgPool;

mod crontabs;

use crontabs::{ConfiguredJobs, build_crontabs};

use crate::{
    jobs::{
        DAILY_TIMEOUT, WEEKLY_TIMEOUT,
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
        ingest_run_recovery::IngestRunRecovery,
        jquants::{MarginIngest, ShortRatioIngest, ShortSaleReportIngest},
        news::NewsAggregation,
        news_content::NewsContentFetch,
        paper_order_filling::PaperOrderFilling,
        prediction::PredictionGrading,
        strategy_task_reconcile::StrategyTaskReconcile,
        trigger_evaluation::TriggerEvaluation,
        twse::TwseIndexIngest,
        us_stock_bars::{UsStockBarsIngest, UsStockHourlyBarsIngest, UsStockMinuteBarsIngest},
        us_stock_master::UsStockMasterIngest,
        valuation::ValuationIngest,
    },
    state::{SchedulerDependencies, SchedulerState},
};

pub const GRAPHILE_WORKER_SCHEMA: &str = "graphile_worker";
const INGEST_RUN_RECOVERY_INTERVAL_MINUTES: u32 = 5;
pub(crate) const NEWS_CONTENT_JOB_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub(crate) const RECOVERABLE_INGEST_JOBS: [(&str, Duration); 26] = [
    (FredIngest::IDENTIFIER, DAILY_TIMEOUT),
    (EodDataKospiIngest::IDENTIFIER, DAILY_TIMEOUT),
    (TwseIndexIngest::IDENTIFIER, DAILY_TIMEOUT),
    (AlphaVantageCalendarIngest::IDENTIFIER, DAILY_TIMEOUT),
    (FredReleaseDatesIngest::IDENTIFIER, DAILY_TIMEOUT),
    (EStatCalendarIngest::IDENTIFIER, DAILY_TIMEOUT),
    (BOJ_CALENDAR_EVENT_INGEST_JOB, DAILY_TIMEOUT),
    (ECB_CALENDAR_EVENT_INGEST_JOB, DAILY_TIMEOUT),
    (FED_CALENDAR_EVENT_INGEST_JOB, DAILY_TIMEOUT),
    (ShortRatioIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShortSaleReportIngest::IDENTIFIER, DAILY_TIMEOUT),
    (MarginIngest::IDENTIFIER, DAILY_TIMEOUT),
    (PredictionGrading::IDENTIFIER, WEEKLY_TIMEOUT),
    (PaperOrderFilling::IDENTIFIER, DAILY_TIMEOUT),
    (DailyBarsIngest::IDENTIFIER, DAILY_TIMEOUT),
    (EarningsScheduleIngest::IDENTIFIER, DAILY_TIMEOUT),
    (FinancialSummaryIngest::IDENTIFIER, DAILY_TIMEOUT),
    (NewsAggregation::IDENTIFIER, DAILY_TIMEOUT),
    (NewsContentFetch::IDENTIFIER, NEWS_CONTENT_JOB_TIMEOUT),
    (ValuationIngest::IDENTIFIER, DAILY_TIMEOUT),
    (EquityMasterIngest::IDENTIFIER, DAILY_TIMEOUT),
    (ShareholdingStructureIngest::IDENTIFIER, DAILY_TIMEOUT),
    (UsStockMasterIngest::IDENTIFIER, DAILY_TIMEOUT),
    (UsStockBarsIngest::IDENTIFIER, DAILY_TIMEOUT),
    (UsStockMinuteBarsIngest::IDENTIFIER, DAILY_TIMEOUT),
    (UsStockHourlyBarsIngest::IDENTIFIER, DAILY_TIMEOUT),
];

pub struct Scheduler {
    worker: Worker,
}

impl Scheduler {
    pub async fn initialize(
        pool: PgPool,
        dependencies: SchedulerDependencies,
        shutdown_signal: impl Future<Output = ()> + Send + 'static,
    ) -> Result<Self, String> {
        let news_content_configured = dependencies.news_content_fetcher.is_some();
        let crontabs = build_crontabs(ConfiguredJobs {
            daily_bars: dependencies.market_daily_bar_source.is_some(),
            fred: dependencies.fred_source.is_some(),
            eoddata_kospi: dependencies.eoddata_kospi_source.is_some(),
            twse: dependencies.twse_source.is_some(),
            e_stat_calendar: dependencies.e_stat_calendar_source.is_some(),
            alpha_vantage_calendar: dependencies.alpha_vantage_calendar_source.is_some(),
            fred_release_dates: dependencies.fred_calendar_event_source.is_some(),
            boj_calendar: dependencies.boj_calendar_source.is_some(),
            ecb_calendar: dependencies.ecb_calendar_source.is_some(),
            fed_calendar: dependencies.fed_calendar_source.is_some(),
            jquants: dependencies.short_selling_source.is_some()
                && dependencies.margin_source.is_some(),
            earnings_schedule: dependencies.earnings_schedule_source.is_some(),
            financial_summary: dependencies.financial_summary_source.is_some(),
            valuation: dependencies.valuation_source.is_some(),
            equity_master: dependencies.equity_master_source.is_some(),
            us_stock_master: dependencies.us_stock_master_source.is_some(),
            us_stock_bars: dependencies.us_stock_bar_source.is_some(),
            shareholding_structure: dependencies.shareholding_structure_source.is_some(),
            strategy_task_reconcile: dependencies.strategy_task_reconcile_enabled,
            news_content: news_content_configured,
        })
        .map_err(|error| error.to_string())?;
        let state = SchedulerState { dependencies };
        let recovery_cron =
            Cron::every_n_minutes::<IngestRunRecovery>(INGEST_RUN_RECOVERY_INTERVAL_MINUTES)
                .map_err(|error| error.to_string())?
                .fill(CrontabFill::minutes(10));
        let worker_options = WorkerOptions::default()
            .pg_pool(pool)
            .schema(GRAPHILE_WORKER_SCHEMA)
            .concurrency(2)
            .use_notification_delivery(false)
            .use_local_time(false)
            .listen_os_shutdown_signals(false)
            .shutdown_signal(shutdown_signal)
            .add_extension(state)
            .define_job::<DailyBarsIngest>()
            .define_job::<NewsAggregation>()
            .define_job::<EarningsScheduleIngest>()
            .define_job::<FinancialSummaryIngest>()
            .define_job::<ValuationIngest>()
            .define_job::<EquityMasterIngest>()
            .define_job::<UsStockMasterIngest>()
            .define_job::<UsStockBarsIngest>()
            .define_job::<UsStockMinuteBarsIngest>()
            .define_job::<UsStockHourlyBarsIngest>()
            .define_job::<ShareholdingStructureIngest>()
            .define_job::<FredIngest>()
            .define_job::<EodDataKospiIngest>()
            .define_job::<TwseIndexIngest>()
            .define_job::<EStatCalendarIngest>()
            .define_job::<AlphaVantageCalendarIngest>()
            .define_job::<FredReleaseDatesIngest>()
            .define_job::<BojCalendarEventIngest>()
            .define_job::<EcbCalendarEventIngest>()
            .define_job::<FedCalendarEventIngest>()
            .define_job::<IngestRunRecovery>()
            .define_job::<ShortRatioIngest>()
            .define_job::<ShortSaleReportIngest>()
            .define_job::<MarginIngest>()
            .define_job::<PredictionGrading>()
            .define_job::<PaperOrderFilling>()
            .define_job::<StrategyTaskReconcile>()
            .define_job::<EventDispatch>()
            .define_job::<TriggerEvaluation>();
        let worker_options = if news_content_configured {
            worker_options.define_job::<NewsContentFetch>()
        } else {
            worker_options
        };
        let worker = worker_options
            .with_crons(crontabs)
            .with_cron(recovery_cron)
            .init()
            .await
            .map_err(|error| error.to_string())?;

        Ok(Self { worker })
    }

    pub async fn run(self) -> Result<(), String> {
        self.worker.run().await.map_err(|error| error.to_string())
    }
}
