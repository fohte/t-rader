use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ListMoversDirection {
    Up,
    Down,
    Abs,
}

impl From<ListMoversDirection> for core_application::market_movers::MarketMoverDirection {
    fn from(direction: ListMoversDirection) -> Self {
        match direction {
            ListMoversDirection::Up => Self::Up,
            ListMoversDirection::Down => Self::Down,
            ListMoversDirection::Abs => Self::Abs,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListMoversParams {
    /// 開始日 (inclusive)。
    pub from: NaiveDate,
    /// 終了日 (inclusive)。
    pub to: NaiveDate,
    /// 値上がり / 値下がり / 騰落率の絶対値で並べる。
    pub direction: ListMoversDirection,
    /// 期間内の平均売買代金の下限 (銘柄ごとの通貨単位)。省略時は下限なし。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_avg_turnover: Option<f64>,
    /// 返却行数 (1〜100)。省略時は 50。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListMoverDto {
    pub instrument_id: String,
    pub name: String,
    /// 小数比率。0.05 は 5% を表す。
    pub change_rate: f64,
    /// 期間内の 1 営業日あたり平均売買代金 (銘柄ごとの通貨単位)。
    pub avg_turnover: f64,
    pub first_seen_at: Option<DateTime<FixedOffset>>,
    pub seen_via: Option<String>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListMoversResult {
    pub movers: Vec<ListMoverDto>,
}
