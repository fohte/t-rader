mod error;
mod ingest;
mod market_price;
mod repository;
mod types;
mod us_stock_ingest;
mod us_stock_minute_bars_downsample;
mod us_stock_source;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::BarsUseCaseError;
pub use repository::{BarsRepository, BarsRepositoryError, SharedBarsRepository};
pub use types::{
    BarsByInstrumentsQuery, BarsQuery, DailyBarAdjustmentFactor, IngestStats, LatestPrices,
    UsStockBarTarget, UsStockBarsIngestStats, UsStockMinuteBarsDownsampleStats,
};
pub use us_stock_source::{
    SharedUsStockBarSource, SkippedUsStockInstrument, UsStockBarPage, UsStockBarQuery,
    UsStockBarSource, UsStockBarSourceError, UsStockSplit, UsStockSplitBatch, UsStockSplitQuery,
};
pub use use_cases::BarsUseCases;

use chrono::{DateTime, Duration, Months, Timelike, Utc};

pub(super) const HOURLY_HISTORY_MONTHS: u32 = 12;

pub(super) fn hourly_history_cutoff(now: DateTime<Utc>) -> DateTime<Utc> {
    now.checked_sub_months(Months::new(HOURLY_HISTORY_MONTHS))
        .unwrap_or(now)
}

pub(super) fn hourly_bar_bucket_start(timestamp: DateTime<Utc>) -> DateTime<Utc> {
    timestamp
        .date_naive()
        .and_hms_opt(timestamp.hour(), 0, 0)
        .map(|timestamp| timestamp.and_utc())
        .unwrap_or(timestamp)
}

// 1 時間足ごとに取得元を切り替え、同じ bucket の重複と欠落を防ぐ。
pub(super) fn hourly_bar_cutover(cutoff: DateTime<Utc>) -> DateTime<Utc> {
    let hour_start = hourly_bar_bucket_start(cutoff);
    if hour_start == cutoff {
        cutoff
    } else {
        hour_start
            .checked_add_signed(Duration::hours(1))
            .unwrap_or(hour_start)
    }
}

pub fn us_stock_hourly_bar_cutover(now: DateTime<Utc>) -> DateTime<Utc> {
    hourly_bar_cutover(hourly_history_cutoff(now))
}

#[cfg(feature = "test-support")]
pub use fake::FakeBarsRepository;

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use rstest::rstest;

    use super::us_stock_hourly_bar_cutover;

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .to_utc()
    }

    #[rstest]
    #[case::cutoff_on_hour("2040-05-31T11:00:00Z", "2039-05-31T11:00:00Z")]
    #[case::cutoff_inside_hour("2040-05-31T11:12:00Z", "2039-05-31T12:00:00Z")]
    fn hourly_read_cutover_uses_the_shared_history_limit(
        #[case] now: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(us_stock_hourly_bar_cutover(date(now)), date(expected));
    }
}
