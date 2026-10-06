mod repository;
mod types;
mod use_cases;

#[cfg(test)]
mod tests;

pub use repository::{IngestStatusRepository, SharedIngestStatusRepository};
pub use types::{
    INGEST_JOBS, IngestJobDefinition, IngestJobStatus, IngestRun, IngestRunHistory, IngestStatus,
    IngestStatusData, IngestStatusJobDate, IngestWorkerJob,
};
pub use use_cases::IngestStatusUseCase;

pub const DAILY_BARS_INGEST_JOB: &str = "daily_bars_ingest";
pub const EARNINGS_SCHEDULE_INGEST_JOB: &str = "earnings_schedule_ingest";
pub const FINANCIAL_SUMMARY_INGEST_JOB: &str = "financial_summary_ingest";
pub const VALUATION_INGEST_JOB: &str = "valuation_ingest";
pub const EQUITY_MASTER_INGEST_JOB: &str = "equity_master_ingest";
pub const US_STOCK_MASTER_INGEST_JOB: &str = "us_stock_master_ingest";
pub const SHAREHOLDING_STRUCTURE_INGEST_JOB: &str = "shareholding_structure_ingest";
pub const FRED_INGEST_JOB: &str = "fred_ingest";
pub const TWSE_INDEX_INGEST_JOB: &str = "twse_index_ingest";
pub const E_STAT_CALENDAR_INGEST_JOB: &str = "e_stat_calendar_ingest";
pub const ALPHA_VANTAGE_CALENDAR_INGEST_JOB: &str = "alpha_vantage_calendar_ingest";
pub const FRED_RELEASE_DATES_INGEST_JOB: &str = "fred_release_dates_ingest";
pub const BOJ_CALENDAR_EVENT_INGEST_JOB: &str = "boj_calendar_event_ingest";
pub const ECB_CALENDAR_EVENT_INGEST_JOB: &str = "ecb_calendar_event_ingest";
pub const FED_CALENDAR_EVENT_INGEST_JOB: &str = "fed_calendar_event_ingest";
pub const SHORT_RATIO_INGEST_JOB: &str = "short_ratio_ingest";
pub const SHORT_SALE_REPORT_INGEST_JOB: &str = "short_sale_report_ingest";
pub const MARGIN_INGEST_JOB: &str = "margin_ingest";
pub const NEWS_AGGREGATION_JOB: &str = "news_aggregation";
pub const NEWS_CONTENT_FETCH_JOB: &str = "news_content_fetch";
pub const PREDICTION_GRADING_JOB: &str = "prediction_grading";
