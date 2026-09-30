//! 戦略実行 MCP の `read_valuation` tool。

use core_application::strategy_scope::StrategyScope;
use core_domain::valuation::Valuation;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::dto::{ReadValuationParams, ReadValuationResult, ValuationDto};
use super::{StrategyServer, internal_error};

impl StrategyServer {
    pub(crate) async fn read_valuation_inner(
        &self,
        // valuation は会社単位の市場データであり戦略に属さないため検索条件に使わない。
        scope: impl Into<StrategyScope>,
        params: ReadValuationParams,
    ) -> Result<ReadValuationResult, McpError> {
        let valuations = self
            .use_cases
            .valuations()
            .find_for_symbol(scope.into(), &params.symbol, params.from, params.to)
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "strategy mcp db error");
                internal_error(format!("database error: {error}"))
            })?;

        Ok(ReadValuationResult {
            symbol: params.symbol,
            items: valuations.into_iter().map(ValuationDto::from).collect(),
        })
    }
}

impl From<Valuation> for ValuationDto {
    fn from(row: Valuation) -> Self {
        Self {
            date: row.date,
            eps: decimal_to_f64(row.eps),
            fwd_eps: decimal_to_f64(row.fwd_eps),
            bps: decimal_to_f64(row.bps),
            roe: decimal_to_f64(row.roe),
            fwd_roe: decimal_to_f64(row.fwd_roe),
            per: decimal_to_f64(row.per),
            fwd_per: decimal_to_f64(row.fwd_per),
            pbr: decimal_to_f64(row.pbr),
            mkt_cap: decimal_to_f64(row.mkt_cap),
        }
    }
}

fn decimal_to_f64(value: Option<Decimal>) -> Option<f64> {
    value.and_then(|value| value.to_f64())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    use super::ValuationDto;
    use core_domain::valuation::Valuation;

    #[test]
    fn converts_domain_valuation_to_mcp_dto() {
        let date = NaiveDate::from_ymd_opt(2042, 4, 17).expect("valid date");
        assert_eq!(
            ValuationDto::from(Valuation {
                code: "ZZZ90".to_string(),
                date,
                eps: Some(Decimal::new(125, 1)),
                fwd_eps: Some(Decimal::new(135, 1)),
                bps: Some(Decimal::new(145, 1)),
                roe: Some(Decimal::new(15, 2)),
                fwd_roe: Some(Decimal::new(18, 2)),
                per: Some(Decimal::new(205, 1)),
                fwd_per: Some(Decimal::new(195, 1)),
                pbr: Some(Decimal::new(12, 1)),
                mkt_cap: Some(Decimal::new(98765, 2)),
            }),
            ValuationDto {
                date,
                eps: Some(12.5),
                fwd_eps: Some(13.5),
                bps: Some(14.5),
                roe: Some(0.15),
                fwd_roe: Some(0.18),
                per: Some(20.5),
                fwd_per: Some(19.5),
                pbr: Some(1.2),
                mkt_cap: Some(987.65),
            },
        );
    }
}
