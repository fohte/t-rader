use core_application::agent_task_client::SharedAgentTaskClient;
use core_application::{
    bars::BarsUseCases, calendar::source::SharedCalendarEventSource,
    calendar::use_cases::CalendarEventUseCases, earnings_schedule::EarningsScheduleUseCases,
    earnings_schedule_source::SharedEarningsScheduleSource, equity_master::EquityMasterUseCases,
    equity_master_source::SharedEquityMasterSource, financial_summary::FinancialSummaryUseCases,
    financial_summary_source::SharedFinancialSummarySource,
    indicator_observation::IndicatorObservationUseCases,
    indicator_observation_source::SharedIndicatorObservationSource,
    ingest_run_log::SharedIngestRunLog, margin::MarginUseCases, margin_source::SharedMarginSource,
    market_daily_bar_source::SharedMarketDailyBarSource, news::NewsUseCases,
    news_aggregator::SharedNewsAggregator, news_content::NewsContentUseCases,
    news_content::SharedNewsContentFetcher, prediction::PredictionUseCases,
    shareholding_structure::ShareholdingStructureUseCases,
    shareholding_structure_source::SharedShareholdingStructureSource,
    short_ratio::ShortRatioUseCases, short_sale_report::ShortSaleReportUseCases,
    short_selling_source::SharedShortSellingSource, strategy_task::StrategyTaskUseCases,
    trigger::TriggerUseCases, us_stock_master::UsStockMasterUseCases,
    us_stock_master_source::SharedUsStockMasterSource, valuation::ValuationUseCases,
    valuation_source::SharedValuationSource,
};

#[derive(Clone)]
pub struct SchedulerDependencies {
    pub bars: BarsUseCases,
    pub calendar_events: CalendarEventUseCases,
    pub boj_calendar_source: Option<SharedCalendarEventSource>,
    pub ecb_calendar_source: Option<SharedCalendarEventSource>,
    pub market_daily_bar_source: Option<SharedMarketDailyBarSource>,
    pub news: NewsUseCases,
    pub news_aggregator: SharedNewsAggregator,
    pub news_content: NewsContentUseCases,
    pub news_content_fetcher: Option<SharedNewsContentFetcher>,
    pub earnings_schedules: EarningsScheduleUseCases,
    pub earnings_schedule_source: Option<SharedEarningsScheduleSource>,
    pub financial_summaries: FinancialSummaryUseCases,
    pub financial_summary_source: Option<SharedFinancialSummarySource>,
    pub equity_master: EquityMasterUseCases,
    pub equity_master_source: Option<SharedEquityMasterSource>,
    pub us_stock_master: UsStockMasterUseCases,
    pub us_stock_master_source: Option<SharedUsStockMasterSource>,
    pub shareholding_structures: ShareholdingStructureUseCases,
    pub shareholding_structure_source: Option<SharedShareholdingStructureSource>,
    pub valuations: ValuationUseCases,
    pub valuation_source: Option<SharedValuationSource>,
    pub indicator_observations: IndicatorObservationUseCases,
    pub ingest_run_log: SharedIngestRunLog,
    pub fred_source: Option<SharedIndicatorObservationSource>,
    pub alpha_vantage_calendar_source: Option<SharedCalendarEventSource>,
    pub fred_calendar_event_source: Option<SharedCalendarEventSource>,
    pub predictions: PredictionUseCases,
    pub short_ratios: ShortRatioUseCases,
    pub short_sale_reports: ShortSaleReportUseCases,
    pub margins: MarginUseCases,
    pub short_selling_source: Option<SharedShortSellingSource>,
    pub margin_source: Option<SharedMarginSource>,
    pub strategy_tasks: StrategyTaskUseCases,
    pub triggers: TriggerUseCases,
    pub agent_task_client: SharedAgentTaskClient,
    pub strategy_task_reconcile_enabled: bool,
}

#[derive(Clone)]
pub(crate) struct SchedulerState {
    pub(super) dependencies: SchedulerDependencies,
}

impl std::fmt::Debug for SchedulerState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SchedulerState")
            .finish_non_exhaustive()
    }
}
