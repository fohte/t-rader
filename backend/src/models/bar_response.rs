use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

use gateway_postgres::entities::bars;

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

impl From<bars::Model> for BarResponse {
    fn from(bar: bars::Model) -> Self {
        Self {
            instrument_id: bar.instrument_id,
            timeframe: bar.timeframe,
            timestamp: bar.timestamp,
            open: bar.open,
            high: bar.high,
            low: bar.low,
            close: bar.close,
            volume: bar.volume,
        }
    }
}
