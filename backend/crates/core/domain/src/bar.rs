use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 時間足の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl Timeframe {
    pub const ALL: [Self; 6] = [
        Self::Daily,
        Self::Minute,
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::Hourly,
        Self::FourHours,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "1d",
            Self::Minute => "1m",
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::Hourly => "1h",
            Self::FourHours => "4h",
        }
    }

    pub const fn bucket_interval(self) -> Option<&'static str> {
        match self {
            Self::Daily | Self::Minute => None,
            Self::FiveMinutes => Some("5 minutes"),
            Self::FifteenMinutes => Some("15 minutes"),
            Self::Hourly => Some("1 hour"),
            Self::FourHours => Some("4 hours"),
        }
    }
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|timeframe| timeframe.as_str() == s)
            .ok_or_else(|| format!("unknown timeframe: {s}"))
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
    /// 価格に適用された調整係数
    pub adjustment_factor: Decimal,
}
