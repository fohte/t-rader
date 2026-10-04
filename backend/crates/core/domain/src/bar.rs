use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 時間足の種類
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timeframe {
    /// 日足
    #[serde(rename = "1d")]
    Daily,
    /// 1 分足
    #[serde(rename = "1m")]
    Minute,
    /// 5 分足
    #[serde(rename = "5m")]
    FiveMinutes,
    /// 15 分足
    #[serde(rename = "15m")]
    FifteenMinutes,
    /// 1 時間足
    #[serde(rename = "1h")]
    Hourly,
    /// 4 時間足
    #[serde(rename = "4h")]
    FourHours,
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Daily => write!(f, "1d"),
            Timeframe::Minute => write!(f, "1m"),
            Timeframe::FiveMinutes => write!(f, "5m"),
            Timeframe::FifteenMinutes => write!(f, "15m"),
            Timeframe::Hourly => write!(f, "1h"),
            Timeframe::FourHours => write!(f, "4h"),
        }
    }
}

impl std::str::FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1d" => Ok(Timeframe::Daily),
            "1m" => Ok(Timeframe::Minute),
            "5m" => Ok(Timeframe::FiveMinutes),
            "15m" => Ok(Timeframe::FifteenMinutes),
            "1h" => Ok(Timeframe::Hourly),
            "4h" => Ok(Timeframe::FourHours),
            other => Err(format!("unknown timeframe: {other}")),
        }
    }
}

/// OHLCV バーデータ
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
