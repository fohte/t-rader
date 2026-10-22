use std::collections::HashMap;
use std::ops::Deref;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use core_domain::bar::Timeframe;
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyBarAdjustmentFactor {
    pub date: NaiveDate,
    pub factor: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsQuery {
    pub instrument_id: String,
    pub timeframe: String,
    pub from: Option<DateTime<FixedOffset>>,
    pub to: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarsByInstrumentsQuery {
    pub instrument_ids: Vec<String>,
    pub timeframe: String,
    pub from: Option<DateTime<FixedOffset>>,
    pub to: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct IngestStats {
    pub days_attempted: usize,
    pub bars_upserted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockBarTarget {
    pub instrument_id: String,
    pub latest_daily_bar: Option<DateTime<Utc>>,
    pub latest_minute_bar: Option<DateTime<Utc>>,
    pub latest_hourly_bar: Option<DateTime<Utc>>,
    pub minute_history_completed_through: Option<DateTime<Utc>>,
    pub hourly_history_completed_through: Option<DateTime<Utc>>,
    pub earliest_daily_bar: Option<DateTime<Utc>>,
    pub earliest_minute_bar: Option<DateTime<Utc>>,
    pub earliest_hourly_bar: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsStockSipBarRefetchTimeframe {
    Daily,
    Hourly,
    Minute,
}

impl UsStockSipBarRefetchTimeframe {
    pub const fn as_timeframe(self) -> Timeframe {
        match self {
            Self::Daily => Timeframe::Daily,
            Self::Hourly => Timeframe::Hourly,
            Self::Minute => Timeframe::Minute,
        }
    }

    pub const fn as_str(self) -> &'static str {
        self.as_timeframe().as_str()
    }
}

impl std::str::FromStr for UsStockSipBarRefetchTimeframe {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "1d" => Ok(Self::Daily),
            "1h" => Ok(Self::Hourly),
            "1m" => Ok(Self::Minute),
            _ => Err(format!("unsupported SIP refetch timeframe: {value}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockSipBarRefetchCursor {
    pub timeframe: UsStockSipBarRefetchTimeframe,
    pub range_from: DateTime<Utc>,
    pub range_to: DateTime<Utc>,
    pub page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockSipBarRefetchTarget {
    pub bars: UsStockBarTarget,
    pub cursor: Option<UsStockSipBarRefetchCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsStockSipBarRefetchConfig {
    pub max_symbols: Option<usize>,
    pub max_rows_per_hour: u64,
}

impl UsStockBarTarget {
    /// 履歴 API は銘柄ごとに初回だけ呼ぶため、完了時刻の有無で判定する。
    pub(crate) fn has_hourly_history_backfill(&self) -> bool {
        self.hourly_history_completed_through.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct UsStockBarsIngestStats {
    pub symbols_attempted: usize,
    pub requests_attempted: usize,
    pub daily_bars_upserted: usize,
    pub minute_bars_upserted: usize,
    pub hourly_bars_upserted: usize,
    pub write_lock_wait_ms: u64,
    pub write_throttle_wait_ms: u64,
    pub skipped_instruments: Vec<super::us_stock_source::SkippedUsStockInstrument>,
    pub interrupted_reason: Option<String>,
    /// 分足取り込み後の古い chunk 圧縮失敗を記録する。
    pub compression_error: Option<String>,
}

impl UsStockBarsIngestStats {
    pub(crate) fn total_rows_upserted(&self) -> u64 {
        u64::try_from(
            self.daily_bars_upserted
                .saturating_add(self.hourly_bars_upserted)
                .saturating_add(self.minute_bars_upserted),
        )
        .unwrap_or(u64::MAX)
    }
}

#[derive(serde::Serialize)]
pub struct UsStockBarsIngestResult {
    #[serde(flatten)]
    pub stats: UsStockBarsIngestStats,
    #[serde(skip)]
    pub(super) _write_lock_guard: Option<Box<dyn super::write_lock::UsStockBarsWriteLockGuard>>,
}

impl UsStockBarsIngestResult {
    pub(super) fn new(
        stats: UsStockBarsIngestStats,
        write_lock_guard: Option<Box<dyn super::write_lock::UsStockBarsWriteLockGuard>>,
    ) -> Self {
        Self {
            stats,
            _write_lock_guard: write_lock_guard,
        }
    }

    pub fn empty() -> Self {
        Self::new(UsStockBarsIngestStats::default(), None)
    }
}

impl Deref for UsStockBarsIngestResult {
    type Target = UsStockBarsIngestStats;

    fn deref(&self) -> &Self::Target {
        &self.stats
    }
}

impl std::fmt::Debug for UsStockBarsIngestResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.stats.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{UsStockBarsIngestResult, UsStockBarsIngestStats};

    #[test]
    fn ingest_result_serializes_stats_as_a_flat_object() {
        let result = UsStockBarsIngestResult::new(
            UsStockBarsIngestStats {
                write_lock_wait_ms: 1234,
                ..UsStockBarsIngestStats::default()
            },
            None,
        );

        assert_eq!(
            serde_json::to_value(result).expect("result serializes"),
            json!({
                "symbols_attempted": 0,
                "requests_attempted": 0,
                "daily_bars_upserted": 0,
                "minute_bars_upserted": 0,
                "hourly_bars_upserted": 0,
                "write_lock_wait_ms": 1234,
                "write_throttle_wait_ms": 0,
                "skipped_instruments": [],
                "interrupted_reason": null,
                "compression_error": null,
            }),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct UsStockMinuteBarsDownsampleStats {
    pub hourly_bars_inserted: u64,
    pub minute_bars_deleted: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LatestPrices {
    /// `priced_at` と同じ観測日の終値だけを含む。
    pub prices: HashMap<String, Decimal>,
    /// `prices` の銘柄に共通する観測日。価格が無ければ `None`。
    pub priced_at: Option<NaiveDate>,
}
