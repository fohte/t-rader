use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub enum QueryDataTimeframe {
    #[serde(rename = "1m")]
    Minute,
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "15m")]
    FifteenMinutes,
    #[serde(rename = "1h")]
    Hourly,
    #[serde(rename = "4h")]
    FourHours,
    #[serde(rename = "1d")]
    #[default]
    Daily,
}

impl From<QueryDataTimeframe> for core_domain::bar::Timeframe {
    fn from(timeframe: QueryDataTimeframe) -> Self {
        match timeframe {
            QueryDataTimeframe::Minute => Self::Minute,
            QueryDataTimeframe::FiveMinutes => Self::FiveMinutes,
            QueryDataTimeframe::FifteenMinutes => Self::FifteenMinutes,
            QueryDataTimeframe::Hourly => Self::Hourly,
            QueryDataTimeframe::FourHours => Self::FourHours,
            QueryDataTimeframe::Daily => Self::Daily,
        }
    }
}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryDataParams {
    /// 対象銘柄コードの配列。1 回の呼び出しで複数銘柄をまとめて取得できる
    /// (最大 100 件、重複不可)
    pub instrument_ids: Vec<String>,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: NaiveDate,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: NaiveDate,
    /// 取得する時間足。省略時は日足 (`1d`)。分足の推定バー数は 1 回あたり 50,000 件まで
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeframe: Option<QueryDataTimeframe>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct BarDto {
    pub timestamp: DateTime<FixedOffset>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

/// 指定した時間足の 1 銘柄分のバー。データが 1 件も無い銘柄は `bars: []` になる
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct InstrumentBarsDto {
    pub instrument_id: String,
    pub bars: Vec<BarDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryDataResult {
    /// `instrument_ids` と同じ順序で返す
    pub results: Vec<InstrumentBarsDto>,
}
