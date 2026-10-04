use chrono::{DateTime, FixedOffset, NaiveDate};
use serde_json::Value;
use uuid::Uuid;

use super::{
    ALPHA_VANTAGE_CALENDAR_INGEST_JOB, DAILY_BARS_INGEST_JOB, EARNINGS_SCHEDULE_INGEST_JOB,
    EQUITY_MASTER_INGEST_JOB, FINANCIAL_SUMMARY_INGEST_JOB, FRED_INGEST_JOB,
    FRED_RELEASE_DATES_INGEST_JOB, MARGIN_INGEST_JOB, NEWS_AGGREGATION_JOB, NEWS_CONTENT_FETCH_JOB,
    PREDICTION_GRADING_JOB, SHAREHOLDING_STRUCTURE_INGEST_JOB, SHORT_RATIO_INGEST_JOB,
    SHORT_SALE_REPORT_INGEST_JOB, VALUATION_INGEST_JOB,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestJobDefinition {
    pub identifier: &'static str,
    pub has_data_date: bool,
}

pub const INGEST_JOBS: [IngestJobDefinition; 15] = [
    IngestJobDefinition {
        identifier: DAILY_BARS_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: EARNINGS_SCHEDULE_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: FINANCIAL_SUMMARY_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: VALUATION_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: EQUITY_MASTER_INGEST_JOB,
        has_data_date: false,
    },
    IngestJobDefinition {
        identifier: SHAREHOLDING_STRUCTURE_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: FRED_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: FRED_RELEASE_DATES_INGEST_JOB,
        has_data_date: false,
    },
    IngestJobDefinition {
        identifier: ALPHA_VANTAGE_CALENDAR_INGEST_JOB,
        // 未来の予定カレンダーなので、最新営業日との日付比較には使わない。
        has_data_date: false,
    },
    IngestJobDefinition {
        identifier: SHORT_RATIO_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: SHORT_SALE_REPORT_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: MARGIN_INGEST_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: NEWS_AGGREGATION_JOB,
        has_data_date: true,
    },
    IngestJobDefinition {
        identifier: NEWS_CONTENT_FETCH_JOB,
        has_data_date: false,
    },
    IngestJobDefinition {
        identifier: PREDICTION_GRADING_JOB,
        has_data_date: false,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestRun {
    pub id: Uuid,
    pub started_at: DateTime<FixedOffset>,
    pub finished_at: Option<DateTime<FixedOffset>>,
    pub status: String,
    pub stats: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestRunHistory {
    pub job: String,
    pub latest_run: IngestRun,
    pub last_succeeded_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestStatusJobDate {
    pub job: String,
    pub latest_data_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestWorkerJob {
    pub id: i64,
    pub job: String,
    pub state: String,
    pub queue_name: Option<String>,
    pub run_at: DateTime<FixedOffset>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IngestStatusData {
    pub run_histories: Vec<IngestRunHistory>,
    pub latest_data_dates: Vec<IngestStatusJobDate>,
    pub worker_jobs: Vec<IngestWorkerJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestJobStatus {
    pub job: String,
    pub latest_run: Option<IngestRun>,
    pub last_succeeded_at: Option<DateTime<FixedOffset>>,
    pub latest_data_date: Option<NaiveDate>,
    pub expected_data_date: Option<NaiveDate>,
    pub worker_jobs: Vec<IngestWorkerJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestStatus {
    pub jobs: Vec<IngestJobStatus>,
}
