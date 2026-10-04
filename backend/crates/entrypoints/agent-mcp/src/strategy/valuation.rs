//! 戦略実行 MCP の `read_valuation` tool。

use core_application::strategy_scope::StrategyScope;
use core_domain::valuation::Valuation;
use rmcp::ErrorData as McpError;

use super::dto::{ReadValuationParams, ReadValuationResult, ValuationDto};
use super::{StrategyServer, decimal_to_f64, reject_foreign_stock_id, valuation_error};

impl StrategyServer {
    pub(crate) async fn read_valuation_inner(
        &self,
        // valuation は会社単位の市場データであり戦略に属さないため検索条件に使わない。
        scope: impl Into<StrategyScope>,
        params: ReadValuationParams,
    ) -> Result<ReadValuationResult, McpError> {
        reject_foreign_stock_id(&params.symbol)?;
        let valuations = self
            .dependencies
            .valuations
            .find_for_symbol(scope.into(), &params.symbol, params.from, params.to)
            .await
            .map_err(valuation_error)?;

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
            eps: row.eps.map(decimal_to_f64),
            fwd_eps: row.fwd_eps.map(decimal_to_f64),
            bps: row.bps.map(decimal_to_f64),
            roe: row.roe.map(decimal_to_f64),
            fwd_roe: row.fwd_roe.map(decimal_to_f64),
            per: row.per.map(decimal_to_f64),
            fwd_per: row.fwd_per.map(decimal_to_f64),
            pbr: row.pbr.map(decimal_to_f64),
            mkt_cap: row.mkt_cap.map(decimal_to_f64),
        }
    }
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
