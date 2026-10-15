use core_application::group_movers::{GroupMoversQuery, GroupMoversUseCaseError};
use core_application::market_movers::{
    MarketMoverDirection, MarketMoversQuery, MarketMoversUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::dto::{
    GroupMoverDto, GroupMoverStockDto, ListGroupMoversParams, ListGroupMoversResult, ListMoverDto,
    ListMoversParams, ListMoversResult,
};
use super::{StrategyServer, internal_error, invalid_params};

const DEFAULT_LIST_MOVERS_LIMIT: u32 = 50;

impl StrategyServer {
    pub(crate) async fn list_movers_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ListMoversParams,
    ) -> Result<ListMoversResult, McpError> {
        let min_avg_turnover = parse_min_avg_turnover(params.min_avg_turnover)?;

        let limit = params.limit.unwrap_or(DEFAULT_LIST_MOVERS_LIMIT);
        let scope = scope.into();
        let rows = self
            .dependencies
            .market_movers
            .list_movers(
                scope,
                MarketMoversQuery {
                    from: params.from,
                    to: params.to,
                    direction: MarketMoverDirection::from(params.direction),
                    min_avg_turnover,
                    markets: params.markets,
                    limit,
                },
            )
            .await
            .map_err(market_movers_use_case_error)?;

        Ok(ListMoversResult {
            movers: rows
                .into_iter()
                .map(|mover| ListMoverDto {
                    instrument_id: mover.instrument_id,
                    name: mover.name,
                    change_rate: super::decimal_to_f64(mover.change_rate),
                    avg_turnover: super::decimal_to_f64(mover.avg_turnover),
                    first_seen_at: mover.first_seen_at,
                    seen_via: mover.seen_via,
                })
                .collect(),
        })
    }

    pub(crate) async fn list_group_movers_inner(
        &self,
        params: ListGroupMoversParams,
    ) -> Result<ListGroupMoversResult, McpError> {
        let min_avg_turnover = parse_min_avg_turnover(params.min_avg_turnover)?;

        let axis_key = params.axis_key;
        let rows = self
            .dependencies
            .group_movers
            .list_group_movers(GroupMoversQuery {
                axis_key: axis_key.clone(),
                from: params.from,
                to: params.to,
                min_avg_turnover,
            })
            .await
            .map_err(group_movers_use_case_error)?;

        Ok(ListGroupMoversResult {
            axis_key,
            groups: rows
                .into_iter()
                .map(|group| GroupMoverDto {
                    group_key: group.group_key,
                    name: group.name,
                    constituent_count: group.constituent_count,
                    median_change_rate: group.median_change_rate.map(super::decimal_to_f64),
                    turnover_weighted_change_rate: group
                        .turnover_weighted_change_rate
                        .map(super::decimal_to_f64),
                    total_avg_turnover: super::decimal_to_f64(group.total_avg_turnover),
                    top_stocks: group
                        .top_stocks
                        .into_iter()
                        .map(|stock| GroupMoverStockDto {
                            stock_id: stock.stock_id,
                            name: stock.name,
                            change_rate: super::decimal_to_f64(stock.change_rate),
                            avg_turnover: super::decimal_to_f64(stock.avg_turnover),
                        })
                        .collect(),
                })
                .collect(),
        })
    }
}

fn group_movers_use_case_error(error: GroupMoversUseCaseError) -> McpError {
    match error {
        GroupMoversUseCaseError::InvalidDateRange => invalid_params("from must be on or before to"),
        GroupMoversUseCaseError::InvalidMinAvgTurnover => {
            invalid_params("min_avg_turnover must be a non-negative number")
        }
        GroupMoversUseCaseError::AxisNotFound(axis_key) => {
            invalid_params(format!("group axis {axis_key} not found"))
        }
        error @ GroupMoversUseCaseError::Query(_) => {
            tracing::error!(error = %error, "strategy mcp group movers query failed");
            internal_error(format!("database error: {error}"))
        }
    }
}

fn parse_min_avg_turnover(value: Option<f64>) -> Result<Decimal, McpError> {
    match value {
        Some(value) if !value.is_finite() => {
            Err(invalid_params("min_avg_turnover must be a finite number"))
        }
        Some(value) => Decimal::from_f64(value)
            .ok_or_else(|| invalid_params("min_avg_turnover is out of range")),
        None => Ok(Decimal::ZERO),
    }
}

fn market_movers_use_case_error(error: MarketMoversUseCaseError) -> McpError {
    match error {
        MarketMoversUseCaseError::InvalidDateRange => {
            invalid_params("from must be on or before to")
        }
        MarketMoversUseCaseError::InvalidLimit => invalid_params("limit must be between 1 and 100"),
        MarketMoversUseCaseError::InvalidMinAvgTurnover => {
            invalid_params("min_avg_turnover must be a non-negative number")
        }
        other => {
            tracing::error!(error = %other, "strategy mcp market movers query failed");
            internal_error(format!("database error: {other}"))
        }
    }
}
