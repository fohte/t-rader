use core_application::screener::{
    ScreenerFilters, ScreenerPage, ScreenerQuery, ScreenerRange, ScreenerSort, ScreenerUseCaseError,
};
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::dto::{ScreenStockDto, ScreenStocksParams, ScreenStocksResult};
use super::{StrategyServer, decimal_to_f64, internal_error, invalid_params};

impl StrategyServer {
    pub(crate) async fn screen_stocks_inner(
        &self,
        params: ScreenStocksParams,
    ) -> Result<ScreenStocksResult, McpError> {
        let query = ScreenerQuery {
            filters: ScreenerFilters {
                markets: params.markets.unwrap_or_default(),
                sectors: params.sectors.unwrap_or_default(),
                per: screener_range(
                    decimal_bound(params.min_per, "min_per")?,
                    decimal_bound(params.max_per, "max_per")?,
                ),
                pbr: screener_range(
                    decimal_bound(params.min_pbr, "min_pbr")?,
                    decimal_bound(params.max_pbr, "max_pbr")?,
                ),
                roe: screener_range(
                    decimal_bound(params.min_roe, "min_roe")?,
                    decimal_bound(params.max_roe, "max_roe")?,
                ),
                dividend_yield: screener_range(
                    decimal_bound(params.min_dividend_yield, "min_dividend_yield")?,
                    decimal_bound(params.max_dividend_yield, "max_dividend_yield")?,
                ),
                market_cap: screener_range(
                    decimal_bound(params.min_market_cap, "min_market_cap")?,
                    decimal_bound(params.max_market_cap, "max_market_cap")?,
                ),
            },
            sort: ScreenerSort {
                field: params.sort_by.map(Into::into).unwrap_or_default(),
                direction: params.sort_order.map(Into::into).unwrap_or_default(),
            },
            page: params.page.unwrap_or(1),
        };

        let page = self
            .dependencies
            .screener
            .screen(query)
            .await
            .map_err(screener_use_case_error)?;

        Ok(page.into())
    }
}

impl From<ScreenerPage> for ScreenStocksResult {
    fn from(page: ScreenerPage) -> Self {
        Self {
            items: page.items.into_iter().map(Into::into).collect(),
            total_count: page.total_count,
            as_of: page.as_of,
            page: page.page,
        }
    }
}

impl From<core_application::screener::ScreenerStock> for ScreenStockDto {
    fn from(stock: core_application::screener::ScreenerStock) -> Self {
        Self {
            id: stock.id,
            name: stock.name,
            market: stock.market,
            sector: stock.sector,
            close: stock.close.map(decimal_to_f64),
            per: stock.per.map(decimal_to_f64),
            pbr: stock.pbr.map(decimal_to_f64),
            roe: stock.roe.map(decimal_to_f64),
            dividend_yield: stock.dividend_yield.map(decimal_to_f64),
            market_cap: stock.market_cap.map(decimal_to_f64),
        }
    }
}

fn decimal_bound(value: Option<f64>, name: &str) -> Result<Option<Decimal>, McpError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if !value.is_finite() {
        return Err(invalid_params(format!("{name} must be a finite number")));
    }
    Decimal::from_f64(value)
        .map(Some)
        .ok_or_else(|| invalid_params(format!("{name} is out of range")))
}

fn screener_range(min: Option<Decimal>, max: Option<Decimal>) -> Option<ScreenerRange> {
    (min.is_some() || max.is_some()).then_some(ScreenerRange { min, max })
}

fn screener_use_case_error(error: ScreenerUseCaseError) -> McpError {
    match error {
        error @ (ScreenerUseCaseError::InvalidFilters(_) | ScreenerUseCaseError::InvalidPage) => {
            invalid_params(error.to_string())
        }
        other => {
            tracing::error!(error = %other, "strategy mcp screener query failed");
            internal_error(format!("database error: {other}"))
        }
    }
}
