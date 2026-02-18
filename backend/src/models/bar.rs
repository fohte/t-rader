use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

use crate::entities::bars;

/// 時間足の種類
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timeframe {
    /// 日足
    #[serde(rename = "1d")]
    Daily,
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Timeframe::Daily => write!(f, "1d"),
        }
    }
}

impl std::str::FromStr for Timeframe {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1d" => Ok(Timeframe::Daily),
            other => Err(format!("unknown timeframe: {other}")),
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

/// models::Bar -> entities::bars::ActiveModel 変換 (upsert 用)
impl From<Bar> for bars::ActiveModel {
    fn from(bar: Bar) -> Self {
        bars::ActiveModel {
            instrument_id: Set(bar.instrument_id),
            timeframe: Set(bar.timeframe.to_string()),
            timestamp: Set(bar.timestamp.fixed_offset()),
            open: Set(bar.open),
            high: Set(bar.high),
            low: Set(bar.low),
            close: Set(bar.close),
            volume: Set(bar.volume),
        }
    }
}

/// entities::bars::Model -> models::Bar 変換 (DB 読み込み用)
///
/// DB の CHECK 制約により不正な timeframe は入らない前提で、
/// パース失敗時は Daily をフォールバックとする。
impl From<bars::Model> for Bar {
    fn from(model: bars::Model) -> Self {
        Bar {
            instrument_id: model.instrument_id,
            timeframe: model.timeframe.parse().unwrap_or(Timeframe::Daily),
            timestamp: model.timestamp.to_utc(),
            open: model.open,
            high: model.high,
            low: model.low,
            close: model.close,
            volume: model.volume,
        }
    }
}
