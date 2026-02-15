use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 時間足の種類
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timeframe {
    /// 日足
    Daily,
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Daily => write!(f, "1d"),
        }
    }
}

/// OHLCV バーデータ (bars テーブルに対応)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    /// 銘柄コード
    pub instrument_id: String,
    /// 時間足
    pub timeframe: Timeframe,
    /// タイムスタンプ
    pub timestamp: DateTime<Utc>,
    /// 始値
    pub open: Decimal,
    /// 高値
    pub high: Decimal,
    /// 安値
    pub low: Decimal,
    /// 終値
    pub close: Decimal,
    /// 出来高
    pub volume: i64,
}
