use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub date: NaiveDate,
    pub source: String,
    pub note: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTrade {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub date: NaiveDate,
    pub source: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTradeCommand {
    pub strategy_id: Uuid,
    pub symbol: String,
    pub side: String,
    pub qty: Decimal,
    pub price: Decimal,
    pub fee: Option<Decimal>,
    pub date: NaiveDate,
    pub source: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeOrder {
    DateAscending,
    DateDescending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeQuery {
    pub strategy_id: Option<Uuid>,
    pub symbol: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub limit: Option<u64>,
    pub order: TradeOrder,
    pub include_note_count: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TradeListItem {
    pub trade: Trade,
    pub note_count: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TradeUpdate {
    pub trade: Trade,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TradeUpdateCommand {
    pub strategy_id: Option<Uuid>,
    pub symbol: Option<String>,
    pub side: Option<String>,
    pub qty: Option<Decimal>,
    pub price: Option<Decimal>,
    pub fee: Option<Decimal>,
    pub date: Option<NaiveDate>,
    pub source: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionSummary {
    pub symbol: String,
    pub qty: Decimal,
    pub avg_cost: Decimal,
    pub cost_basis: Decimal,
    pub realized_pnl: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerformanceSummary {
    pub strategy_id: Option<Uuid>,
    pub trade_count: i64,
    pub realized_pnl: Decimal,
    pub positions: Vec<PositionSummary>,
}
