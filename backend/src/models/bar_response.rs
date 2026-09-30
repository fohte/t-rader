use chrono::{DateTime, FixedOffset};
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Bar)]
pub struct BarResponse {
    pub instrument_id: String,
    pub timeframe: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub timestamp: DateTime<FixedOffset>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: i64,
}

impl From<Bar> for BarResponse {
    fn from(bar: Bar) -> Self {
        Self {
            instrument_id: bar.instrument_id,
            timeframe: bar.timeframe.to_string(),
            timestamp: bar.timestamp.fixed_offset(),
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
        }
    }
}
