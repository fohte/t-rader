use chrono::NaiveDate;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortfolioHistoryQuery {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioHistoryPoint {
    pub date: NaiveDate,
    pub total_assets: Decimal,
    pub stock_market_value: Decimal,
    pub cash: Decimal,
    pub investable_amount: Decimal,
}
