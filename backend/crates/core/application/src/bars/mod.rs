mod clock;
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

use chrono::{DateTime, Months, Timelike, Utc};

pub(super) const HOURLY_HISTORY_MONTHS: u32 = 12;

pub fn us_stock_hourly_history_cutoff(now: DateTime<Utc>) -> DateTime<Utc> {
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

#[cfg(feature = "test-support")]
pub use fake::FakeBarsRepository;
