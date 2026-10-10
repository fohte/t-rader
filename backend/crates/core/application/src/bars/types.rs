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
    pub earliest_daily_bar: Option<DateTime<Utc>>,
    pub earliest_minute_bar: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct UsStockBarsIngestStats {
    pub symbols_attempted: usize,
    pub requests_attempted: usize,
    pub daily_bars_upserted: usize,
    pub minute_bars_upserted: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LatestPrices {
    /// `priced_at` と同じ観測日の終値だけを含む。
    pub prices: HashMap<String, Decimal>,
    /// `prices` の銘柄に共通する観測日。価格が無ければ `None`。
    pub priced_at: Option<NaiveDate>,
}
