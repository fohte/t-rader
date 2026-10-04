use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate};
use serde_json::json;
use uuid::Uuid;

use crate::ingest_status::{
    IngestJobStatus, IngestRun, IngestRunHistory, IngestStatus, IngestStatusData,
    IngestStatusJobDate, IngestStatusRepository, IngestStatusUseCase, IngestWorkerJob,
};
use crate::persistence::PersistenceError;

struct FakeIngestStatusRepository(IngestStatusData);

#[async_trait]
impl IngestStatusRepository for FakeIngestStatusRepository {
    async fn read(&self) -> Result<IngestStatusData, PersistenceError> {
        Ok(self.0.clone())
    }
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid fixture date")
}

fn timestamp(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).expect("valid fixture timestamp")
}

#[tokio::test]
async fn get_returns_registered_jobs_with_run_data_dates_and_queue_jobs() {
    let repository = Arc::new(FakeIngestStatusRepository(IngestStatusData {
        run_histories: vec![IngestRunHistory {
            job: "daily_bars_ingest".into(),
            latest_run: IngestRun {
                id: Uuid::from_u128(1),
                started_at: timestamp("2030-06-06T12:00:00Z"),
                finished_at: Some(timestamp("2030-06-06T12:01:00Z")),
                status: "succeeded".into(),
                stats: Some(json!({"rows": 4})),
                error: None,
            },
            last_succeeded_at: Some(timestamp("2030-06-06T12:01:00Z")),
        }],
        latest_data_dates: vec![IngestStatusJobDate {
            job: "daily_bars_ingest".into(),
            latest_data_date: Some(date(2030, 6, 5)),
        }],
        worker_jobs: vec![IngestWorkerJob {
            id: 2,
            job: "daily_bars_ingest".into(),
            state: "failed".into(),
            queue_name: Some("sample-queue".into()),
            run_at: timestamp("2030-06-06T12:02:00Z"),
            attempts: 3,
            max_attempts: 3,
            last_error: Some("sample failure".into()),
        }],
    }));
    let use_case = IngestStatusUseCase::new(repository);

    let actual = use_case
        .get(date(2030, 6, 9))
        .await
        .expect("read ingest status");

    assert_eq!(
        actual,
        IngestStatus {
            jobs: vec![
                IngestJobStatus {
                    job: "daily_bars_ingest".into(),
                    latest_run: Some(IngestRun {
                        id: Uuid::from_u128(1),
                        started_at: timestamp("2030-06-06T12:00:00Z"),
                        finished_at: Some(timestamp("2030-06-06T12:01:00Z")),
                        status: "succeeded".into(),
                        stats: Some(json!({"rows": 4})),
                        error: None,
                    }),
                    last_succeeded_at: Some(timestamp("2030-06-06T12:01:00Z")),
                    latest_data_date: Some(date(2030, 6, 5)),
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: vec![IngestWorkerJob {
                        id: 2,
                        job: "daily_bars_ingest".into(),
                        state: "failed".into(),
                        queue_name: Some("sample-queue".into()),
                        run_at: timestamp("2030-06-06T12:02:00Z"),
                        attempts: 3,
                        max_attempts: 3,
                        last_error: Some("sample failure".into()),
                    }],
                },
                IngestJobStatus {
                    job: "earnings_schedule_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "financial_summary_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "valuation_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "equity_master_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: None,
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "shareholding_structure_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "fred_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "fred_release_dates_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: None,
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "alpha_vantage_calendar_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: None,
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "short_ratio_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "short_sale_report_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "margin_ingest".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "news_aggregation".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: Some(date(2030, 6, 7)),
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "news_content_fetch".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: None,
                    worker_jobs: Vec::new(),
                },
                IngestJobStatus {
                    job: "prediction_grading".into(),
                    latest_run: None,
                    last_succeeded_at: None,
                    latest_data_date: None,
                    expected_data_date: None,
                    worker_jobs: Vec::new(),
                },
            ],
        },
    );
}
