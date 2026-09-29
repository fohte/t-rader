use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct Strategy {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewStrategy {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateStrategyCommand {
    pub name: String,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StrategyUpdateCommand {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvestableAmount {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub amount_jpy: Decimal,
    pub effective_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewInvestableAmount {
    pub id: Uuid,
    pub strategy_id: Uuid,
    pub amount_jpy: Decimal,
    pub effective_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrategySummary {
    pub id: Uuid,
    pub name: String,
    pub updated_at: DateTime<FixedOffset>,
    pub unread_card_count: u64,
}
