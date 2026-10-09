use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlacePaperOrderParams {
    /// 日本株の銘柄 ID。
    pub stock_id: String,
    /// `buy` または `sell`。
    pub side: String,
    /// 100 株単位の注文株数。
    pub qty: i64,
    /// 注文の根拠となったノートの版 ID。
    pub note_version_id: Uuid,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct PlacePaperOrderResult {
    pub order_id: Uuid,
    pub stock_id: String,
    pub side: String,
    pub qty: i64,
    pub note_version_id: Uuid,
    pub ordered_at: DateTime<FixedOffset>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PaperTradePositionDto {
    pub stock_id: String,
    pub qty: i64,
    pub avg_cost_jpy: f64,
    pub current_price_jpy: f64,
    pub market_value_jpy: f64,
    pub unrealized_pnl_jpy: f64,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PaperTradeOrderDto {
    pub order_id: Uuid,
    pub stock_id: String,
    pub side: String,
    pub qty: i64,
    pub note_version_id: Uuid,
    pub ordered_at: DateTime<FixedOffset>,
    pub outcome: Option<String>,
    pub fill_date: Option<NaiveDate>,
    pub fill_price_jpy: Option<f64>,
    pub reject_reason: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadPaperPortfolioParams {
    /// `read_paper_stats` から取得した口座 ID。省略時は接続元 task の口座を読む。
    pub account_id: Option<Uuid>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadPaperPortfolioResult {
    pub account_id: Uuid,
    pub account_name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub started_on: NaiveDate,
    pub as_of: NaiveDate,
    pub initial_cash_jpy: f64,
    pub cash_jpy: f64,
    pub positions: Vec<PaperTradePositionDto>,
    pub orders: Vec<PaperTradeOrderDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PaperTradeAccountStatsDto {
    pub account_id: Uuid,
    pub account_name: String,
    pub strategy_id: Uuid,
    pub purpose: String,
    pub started_on: NaiveDate,
    pub initial_cash_jpy: f64,
    pub benchmark_stock_id: Option<String>,
    pub as_of: NaiveDate,
    pub total_assets_jpy: f64,
    pub return_since_start: f64,
    pub benchmark_return: Option<f64>,
    pub closed_trade_count: usize,
    pub win_rate: Option<f64>,
    pub average_win_excess_return: Option<f64>,
    pub average_loss_excess_return: Option<f64>,
    pub unrealized_pnl_jpy: f64,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadPaperStatsResult {
    pub accounts: Vec<PaperTradeAccountStatsDto>,
}
