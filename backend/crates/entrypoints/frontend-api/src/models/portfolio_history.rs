use chrono::NaiveDate;
use core_application::portfolio_history::PortfolioHistoryPoint;
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct PortfolioHistoryPointResponse {
    pub date: NaiveDate,
    pub total_assets: Decimal,
    pub stock_market_value: Decimal,
    pub cash: Decimal,
    pub investable_amount: Decimal,
}

impl From<PortfolioHistoryPoint> for PortfolioHistoryPointResponse {
    fn from(point: PortfolioHistoryPoint) -> Self {
        Self {
            date: point.date,
            total_assets: point.total_assets,
            stock_market_value: point.stock_market_value,
            cash: point.cash,
            investable_amount: point.investable_amount,
        }
    }
}
