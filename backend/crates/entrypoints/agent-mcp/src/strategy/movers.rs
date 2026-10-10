use core_application::market_movers::{
    MarketMoverDirection, MarketMoversQuery, MarketMoversUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

use super::dto::{ListMoverDto, ListMoversParams, ListMoversResult};
use super::{StrategyServer, internal_error, invalid_params};

const DEFAULT_LIST_MOVERS_LIMIT: u32 = 50;

impl StrategyServer {
    pub(crate) async fn list_movers_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: ListMoversParams,
    ) -> Result<ListMoversResult, McpError> {
        let min_avg_turnover = match params.min_avg_turnover {
            Some(value) if !value.is_finite() => {
                return Err(invalid_params("min_avg_turnover must be a finite number"));
            }
            Some(value) => Decimal::from_f64(value)
                .ok_or_else(|| invalid_params("min_avg_turnover is out of range"))?,
            None => Decimal::ZERO,
        };

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
