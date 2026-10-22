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
mod write_lock;
mod write_throttle;

#[cfg(feature = "test-support")]
mod fake;

pub use error::{BarsUseCaseError, UsStockBarsIngestError};
pub use repository::{BarsRepository, BarsRepositoryError, SharedBarsRepository};
pub use types::{
    BarsByInstrumentsQuery, BarsQuery, DailyBarAdjustmentFactor, IngestStats, LatestPrices,
    US_STOCK_MINUTE_BARS_DOWNSAMPLE_PROCESSING_BUDGET, UsStockBarTarget, UsStockBarsIngestResult,
    UsStockBarsIngestStats, UsStockMinuteBarsDownsampleConfig, UsStockMinuteBarsDownsampleStats,
    UsStockSipBarRefetchConfig, UsStockSipBarRefetchCursor, UsStockSipBarRefetchTarget,
    UsStockSipBarRefetchTimeframe,
};
pub use us_stock_source::{
    SharedUsStockBarSource, SkippedUsStockInstrument, UsStockBarPage, UsStockBarQuery,
    UsStockBarSource, UsStockBarSourceError, UsStockSplit, UsStockSplitBatch, UsStockSplitQuery,
};
pub use use_cases::{BarsUseCases, UsStockBarsIngestUseCases};
pub use write_lock::{
    SharedSipRefetchLease, SharedUsStockBarsWriteLock, UsStockBarsWriteLock,
    UsStockBarsWriteLockGuard,
};

use chrono::{DateTime, Duration, Months, Timelike, Utc};

pub(super) const HOURLY_HISTORY_MONTHS: u32 = 12;
pub(super) const US_STOCK_SIP_DELAY_MINUTES: i64 = 16;

/// 日足取り込み後の stats 記録に時間を残すための上限。
pub const US_STOCK_DAILY_BARS_MAX_RUN_DURATION: std::time::Duration =
    std::time::Duration::from_secs(90 * 60);

/// 分足・時間足の取り込み後に圧縮と stats 記録の時間を残すための上限。
pub const US_STOCK_INTRADAY_BARS_MAX_RUN_DURATION: std::time::Duration =
    std::time::Duration::from_secs(45 * 60);

pub const DEFAULT_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR: u64 = 1_000_000;
pub const MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR: u64 = 10_000;

pub fn us_stock_hourly_history_cutoff(now: DateTime<Utc>) -> DateTime<Utc> {
    now.checked_sub_months(Months::new(HOURLY_HISTORY_MONTHS))
        .unwrap_or(now)
}

pub(super) fn us_stock_sip_bars_history_cutoff(now: DateTime<Utc>) -> DateTime<Utc> {
    let cutoff = now
        .checked_sub_signed(Duration::minutes(US_STOCK_SIP_DELAY_MINUTES))
        .unwrap_or(now);
    cutoff
        .with_second(0)
        .and_then(|cutoff| cutoff.with_nanosecond(0))
        .unwrap_or(cutoff)
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
