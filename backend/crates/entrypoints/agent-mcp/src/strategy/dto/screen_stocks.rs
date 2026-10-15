use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenStocksSortBy {
    Id,
    Name,
    Market,
    Sector,
    Close,
    Per,
    Pbr,
    Roe,
    DividendYield,
    MarketCap,
}

impl From<ScreenStocksSortBy> for core_application::screener::ScreenerSortField {
    fn from(field: ScreenStocksSortBy) -> Self {
        match field {
            ScreenStocksSortBy::Id => Self::Id,
            ScreenStocksSortBy::Name => Self::Name,
            ScreenStocksSortBy::Market => Self::Market,
            ScreenStocksSortBy::Sector => Self::Sector,
            ScreenStocksSortBy::Close => Self::Close,
            ScreenStocksSortBy::Per => Self::Per,
            ScreenStocksSortBy::Pbr => Self::Pbr,
            ScreenStocksSortBy::Roe => Self::Roe,
            ScreenStocksSortBy::DividendYield => Self::DividendYield,
            ScreenStocksSortBy::MarketCap => Self::MarketCap,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScreenStocksSortOrder {
    Asc,
    Desc,
}

impl From<ScreenStocksSortOrder> for core_application::screener::ScreenerSortDirection {
    fn from(direction: ScreenStocksSortOrder) -> Self {
        match direction {
            ScreenStocksSortOrder::Asc => Self::Asc,
            ScreenStocksSortOrder::Desc => Self::Desc,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenStocksParams {
    /// 市場区分 (`stock.market`) の完全一致。省略時は全市場を対象にする。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markets: Option<Vec<String>>,
    /// 東証 33 業種グループの key。省略時は全業種を対象にする。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sectors: Option<Vec<String>>,
    /// PER の下限。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_per: Option<f64>,
    /// PER の上限。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_per: Option<f64>,
    /// PBR の下限。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_pbr: Option<f64>,
    /// PBR の上限。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pbr: Option<f64>,
    /// ROE の下限 (パーセント)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_roe: Option<f64>,
    /// ROE の上限 (パーセント)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_roe: Option<f64>,
    /// 予想配当利回りの下限 (パーセント)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_dividend_yield: Option<f64>,
    /// 予想配当利回りの上限 (パーセント)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_dividend_yield: Option<f64>,
    /// 時価総額の下限 (億円)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_market_cap: Option<f64>,
    /// 時価総額の上限 (億円)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_market_cap: Option<f64>,
    /// 並び替える列。省略時は `market_cap`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<ScreenStocksSortBy>,
    /// 並び順。省略時は `desc`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<ScreenStocksSortOrder>,
    /// 1 始まりのページ番号。省略時は 1。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ScreenStockDto {
    pub id: String,
    pub name: String,
    pub market: Option<String>,
    pub sector: Option<String>,
    pub close: Option<f64>,
    pub per: Option<f64>,
    pub pbr: Option<f64>,
    pub roe: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub market_cap: Option<f64>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ScreenStocksResult {
    pub items: Vec<ScreenStockDto>,
    pub total_count: i64,
    pub as_of: Option<NaiveDate>,
    pub page: u32,
}
