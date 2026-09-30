use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, NaiveDate};
use core_domain::short_ratio::ShortRatio;

use crate::daily_bar_source::DateRange;
use crate::short_ratio::{FakeShortRatioRepository, ShortRatioUseCases};
use crate::short_selling_source::{ShortSellingSource, ShortSellingSourceError};

#[derive(Default)]
struct FakeSource {
    range: Option<DateRange>,
    ratios: HashMap<NaiveDate, Vec<ShortRatio>>,
    failed_dates: HashSet<NaiveDate>,
    requested_dates: Mutex<Vec<NaiveDate>>,
}

#[async_trait]
impl ShortSellingSource for FakeSource {
    async fn fetch_short_sale_reports(
        &self,
        _disc_date: NaiveDate,
    ) -> Result<Vec<core_domain::short_sale_report::ShortSaleReport>, ShortSellingSourceError> {
        Ok(Vec::new())
    }

    async fn fetch_short_ratios(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<ShortRatio>, ShortSellingSourceError> {
        self.requested_dates.lock().unwrap().push(date);
        if self.failed_dates.contains(&date) {
            return Err(ShortSellingSourceError::Failed(
                "synthetic fetch failure".into(),
            ));
        }
        Ok(self.ratios.get(&date).cloned().unwrap_or_default())
    }

    fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
        self.range.clone()
    }
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 1, day).unwrap()
}

fn ratio(date: NaiveDate, value: i64) -> ShortRatio {
    ShortRatio {
        date,
        sector33_code: "T001".into(),
        sell_excluding_short_value: Some(value.into()),
        short_with_restriction_value: Some(value.into()),
        short_without_restriction_value: Some(value.into()),
    }
}

fn use_cases(repository: Arc<FakeShortRatioRepository>) -> ShortRatioUseCases {
    ShortRatioUseCases::new(repository)
}

#[tokio::test]
async fn skips_ingest_when_source_has_no_fetchable_range() {
    let repository = Arc::new(FakeShortRatioRepository::new());
    let source = FakeSource::default();
    let stats = use_cases(repository.clone())
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
    let endpoint_start = NaiveDate::from_ymd_opt(2008, 11, 5).unwrap();
    let today = endpoint_start + Duration::days(1);
    let repository = Arc::new(FakeShortRatioRepository::new());
    let source = FakeSource {
        range: Some(DateRange {
            from: endpoint_start - Duration::days(10),
            to: today,
        }),
        ratios: HashMap::from([(endpoint_start, vec![ratio(endpoint_start, 10)])]),
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
            super::ShortRatioIngestStats {
                days_fetched: 1,
                rows_upserted: 1,
            },
            vec![endpoint_start, today],
            vec![ratio(endpoint_start, 10)],
        ),
    );
}

#[tokio::test]
async fn resumes_seven_days_before_latest_saved_date() {
    let endpoint_start = NaiveDate::from_ymd_opt(2008, 11, 5).unwrap();
    let latest = endpoint_start + Duration::days(20);
    let resume_from = latest - Duration::days(7);
    let today = resume_from + Duration::days(1);
    let repository = Arc::new(FakeShortRatioRepository::new());
    repository.rows.lock().unwrap().push(ratio(latest, 10));
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
            super::ShortRatioIngestStats {
                days_fetched: 1,
                rows_upserted: 0,
            },
            vec![resume_from, resume_from + Duration::days(1)],
            vec![ratio(latest, 10)],
        ),
    );
}

#[tokio::test]
async fn clamps_resume_date_to_provider_range() {
    let endpoint_start = NaiveDate::from_ymd_opt(2008, 11, 5).unwrap();
    let latest = endpoint_start + Duration::days(2);
    let floor = endpoint_start + Duration::days(1);
    let today = floor + Duration::days(1);
    let repository = Arc::new(FakeShortRatioRepository::new());
    repository.rows.lock().unwrap().push(ratio(latest, 10));
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
    let endpoint_start = NaiveDate::from_ymd_opt(2008, 11, 5).unwrap();
    let repository = Arc::new(FakeShortRatioRepository::new());
    repository.fail_upserts_with("synthetic write failure");
    let source = FakeSource {
        range: Some(DateRange {
            from: endpoint_start,
            to: endpoint_start,
        }),
        ratios: HashMap::from([(endpoint_start, vec![ratio(endpoint_start, 10)])]),
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
