use chrono::NaiveDate;
use core_application::screener::{ScreenerPage, ScreenerStock};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScreenerResponse {
    pub items: Vec<ScreenerStockResponse>,
    pub total_count: i64,
    pub as_of: Option<NaiveDate>,
    pub page: u32,
}

impl From<ScreenerPage> for ScreenerResponse {
    fn from(page: ScreenerPage) -> Self {
        Self {
            items: page.items.into_iter().map(Into::into).collect(),
            total_count: page.total_count,
            as_of: page.as_of,
            page: page.page,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ScreenerStockResponse {
    pub id: String,
    pub name: String,
    pub market: Option<String>,
    pub sector: Option<String>,
    pub close: Option<Decimal>,
    pub per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub dividend_yield: Option<Decimal>,
    pub market_cap: Option<Decimal>,
}

impl From<ScreenerStock> for ScreenerStockResponse {
    fn from(stock: ScreenerStock) -> Self {
        Self {
            id: stock.id,
            name: stock.name,
            market: stock.market,
            sector: stock.sector,
            close: stock.close,
            per: stock.per,
            pbr: stock.pbr,
            roe: stock.roe,
            dividend_yield: stock.dividend_yield,
            market_cap: stock.market_cap,
        }
    }
}
