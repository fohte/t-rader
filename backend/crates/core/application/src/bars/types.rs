use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
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
    pub hourly_history_completed_through: Option<DateTime<Utc>>,
    pub earliest_daily_bar: Option<DateTime<Utc>>,
    pub earliest_minute_bar: Option<DateTime<Utc>>,
    pub earliest_hourly_bar: Option<DateTime<Utc>>,
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
    pub skipped_instruments: Vec<super::us_stock_source::SkippedUsStockInstrument>,
    pub interrupted_reason: Option<String>,
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
