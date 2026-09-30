use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use core_domain::short_sale_report::ShortSaleReport;

use crate::daily_bar_source::DateRange;
use crate::short_sale_report::{FakeShortSaleReportRepository, ShortSaleReportUseCases};
use crate::short_selling_source::{ShortSellingSource, ShortSellingSourceError};

#[derive(Default)]
struct FakeSource {
    range: Option<DateRange>,
    reports: HashMap<NaiveDate, Vec<ShortSaleReport>>,
    failed_dates: HashSet<NaiveDate>,
    requested_dates: Mutex<Vec<NaiveDate>>,
}

#[async_trait]
impl ShortSellingSource for FakeSource {
    async fn fetch_short_sale_reports(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<ShortSaleReport>, ShortSellingSourceError> {
        self.requested_dates.lock().unwrap().push(date);
        if self.failed_dates.contains(&date) {
            return Err(ShortSellingSourceError::Failed(
                "synthetic fetch failure".into(),
            ));
        }
        Ok(self.reports.get(&date).cloned().unwrap_or_default())
    }

    async fn fetch_short_ratios(
        &self,
        _date: NaiveDate,
    ) -> Result<Vec<core_domain::short_ratio::ShortRatio>, ShortSellingSourceError> {
        Ok(Vec::new())
    }

    fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
        self.range.clone()
    }
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2013, 11, day).unwrap()
}

fn report(disc_date: NaiveDate, ratio: i64) -> ShortSaleReport {
    ShortSaleReport {
        disc_date,
        calc_date: disc_date,
        code: "X1234".into(),
        ss_name: "架空報告者".into(),
        ss_addr: "架空住所".into(),
        dic_name: "架空委託者".into(),
        dic_addr: "架空住所".into(),
        fund_name: "架空ファンド".into(),
        short_position_ratio: ratio.into(),
        short_position_shares: ratio,
        short_position_units: ratio,
        prev_report_date: None,
        prev_report_ratio: None,
        notes: String::new(),
    }
}

fn use_cases(repository: Arc<FakeShortSaleReportRepository>) -> ShortSaleReportUseCases {
    ShortSaleReportUseCases::new(repository)
}

#[tokio::test]
async fn skips_ingest_when_source_has_no_fetchable_range() {
    let repository = Arc::new(FakeShortSaleReportRepository::new());
    let source = FakeSource::default();
    let stats = use_cases(repository)
        .ingest(&source, date(10))
        .await
        .expect("ingest succeeds");

    assert_eq!(
        (stats, source.requested_dates.lock().unwrap().clone()),
        (Default::default(), Vec::new()),
    );
}

#[tokio::test]
async fn starts_at_endpoint_floor_and_stops_after_fetch_failure() {
    let endpoint_start = NaiveDate::from_ymd_opt(2013, 11, 7).unwrap();
    let today = endpoint_start + Duration::days(1);
    let repository = Arc::new(FakeShortSaleReportRepository::new());
    let source = FakeSource {
        range: Some(DateRange {
            from: endpoint_start - Duration::days(10),
            to: today,
        }),
        reports: HashMap::from([(endpoint_start, vec![report(endpoint_start, 10)])]),
        failed_dates: HashSet::from([today]),
        ..FakeSource::default()
    };

    let stats = use_cases(repository.clone())
        .ingest(&source, today)
        .await
        .expect("fetch failure ends the cycle");

    assert_eq!(
        (
            stats,
            source.requested_dates.lock().unwrap().clone(),
            repository.rows.lock().unwrap().clone(),
        ),
        (
            super::ShortSaleReportIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            },
            vec![endpoint_start, today],
            vec![report(endpoint_start, 10)],
        ),
    );
}

#[tokio::test]
async fn resumes_seven_days_before_latest_saved_date() {
    let endpoint_start = NaiveDate::from_ymd_opt(2013, 11, 7).unwrap();
    let latest = endpoint_start + Duration::days(20);
    let resume_from = latest - Duration::days(7);
    let today = resume_from + Duration::days(1);
    let repository = Arc::new(FakeShortSaleReportRepository::new());
    repository.rows.lock().unwrap().push(report(latest, 10));
    let source = FakeSource {
        range: Some(DateRange {
            from: endpoint_start,
            to: today,
        }),
        failed_dates: HashSet::from([resume_from + Duration::days(1)]),
        ..FakeSource::default()
    };

    let stats = use_cases(repository.clone())
        .ingest(&source, today)
        .await
        .expect("fetch failure ends the cycle");

    assert_eq!(
        (
            stats,
            source.requested_dates.lock().unwrap().clone(),
            repository.rows.lock().unwrap().clone(),
        ),
        (
            super::ShortSaleReportIngestStats {
                days_fetched: 1,
                rows_upserted: 0,
            },
            vec![resume_from, resume_from + Duration::days(1)],
            vec![report(latest, 10)],
        ),
    );
}

#[tokio::test]
async fn clamps_resume_date_to_provider_range() {
    let endpoint_start = NaiveDate::from_ymd_opt(2013, 11, 7).unwrap();
    let latest = endpoint_start + Duration::days(2);
    let floor = endpoint_start + Duration::days(1);
    let today = floor + Duration::days(1);
    let repository = Arc::new(FakeShortSaleReportRepository::new());
    repository.rows.lock().unwrap().push(report(latest, 10));
    let source = FakeSource {
        range: Some(DateRange {
            from: floor,
            to: today,
        }),
        failed_dates: HashSet::from([floor + Duration::days(1)]),
        ..FakeSource::default()
    };

    let stats = use_cases(repository.clone())
        .ingest(&source, today)
        .await
        .expect("fetch failure ends the cycle");

    assert_eq!(
        (
            stats.days_fetched,
            source.requested_dates.lock().unwrap().clone()
        ),
        (1, vec![floor, floor + Duration::days(1)]),
    );
}

#[tokio::test]
async fn returns_repository_error_when_saving_fails() {
    let endpoint_start = NaiveDate::from_ymd_opt(2013, 11, 7).unwrap();
    let repository = Arc::new(FakeShortSaleReportRepository::new());
    repository.fail_upserts_with("synthetic write failure");
    let source = FakeSource {
        range: Some(DateRange {
            from: endpoint_start,
            to: endpoint_start,
        }),
        reports: HashMap::from([(endpoint_start, vec![report(endpoint_start, 10)])]),
        ..FakeSource::default()
    };

    let result = use_cases(repository)
        .ingest(&source, endpoint_start)
        .await
        .map(|_| ());

    assert_eq!(
        result.map_err(|error| error.to_string()),
        Err("synthetic write failure".into())
    );
}
