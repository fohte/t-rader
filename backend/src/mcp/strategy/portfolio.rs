//! 口座全体 (全戦略横断) と接続元戦略、両方のポートフォリオ集計の inner method 実装。
//!
//! 戦略は「お金の区分」であり分析は口座全体を見る、という設計判断から account 側の集計は
//! 全戦略横断の trade を対象にする。そのうえで接続元 strategy_id 自身のスライスも
//! 追加で返す。両スコープとも保有銘柄の直近終値で時価評価する。

use std::collections::{BTreeSet, HashMap};

use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use rust_decimal::Decimal;

use super::dto::{
    PortfolioPositionDto, PortfolioScopeDto, ReadPortfolioResult, StrategyPortfolioScopeDto,
};
use super::{StrategyServer, decimal_to_f64, strategy_use_case_error_to_mcp, trade_error};

impl StrategyServer {
    pub(crate) async fn read_portfolio_inner(
        &self,
        scope: impl Into<StrategyScope>,
    ) -> Result<ReadPortfolioResult, McpError> {
        let scope = scope.into();
        let strategy_id = scope.id();
        let account_summary = self
            .dependencies
            .trades
            .summary(None)
            .await
            .map_err(trade_error)?;
        let strategy_summary = self
            .dependencies
            .trades
            .summary(Some(strategy_id))
            .await
            .map_err(trade_error)?;

        let mut symbols: BTreeSet<String> = BTreeSet::new();
        symbols.extend(account_summary.positions.iter().map(|p| p.symbol.clone()));
        symbols.extend(strategy_summary.positions.iter().map(|p| p.symbol.clone()));
        let symbols: Vec<String> = symbols.into_iter().collect();

        let prices = self
            .dependencies
            .bars
            .fetch_latest_prices(self.dependencies.daily_bar_source.as_deref(), &symbols)
            .await;

        let account_positions = to_position_dtos(account_summary.positions, &prices.prices);
        let account = PortfolioScopeDto {
            trade_count: account_summary.trade_count,
            realized_pnl: decimal_to_f64(account_summary.realized_pnl),
            market_value: sum_market_value(&account_positions),
            positions: account_positions,
        };

        let strategy_cost_basis: Decimal = strategy_summary
            .positions
            .iter()
            .map(|p| p.cost_basis)
            .sum();
        let strategy_realized_pnl = strategy_summary.realized_pnl;
        let strategy_positions = to_position_dtos(strategy_summary.positions, &prices.prices);

        let investable_amount_row = self
            .dependencies
            .strategies
            .current_investable_amount(scope)
            .await
            .map_err(strategy_use_case_error_to_mcp)?;
        let unused_investable_amount = super::unused_investable_amount(
            investable_amount_row.as_ref().map(|row| row.amount_jpy),
            strategy_realized_pnl,
            strategy_cost_basis,
        )
        .map(decimal_to_f64);

        let strategy = StrategyPortfolioScopeDto {
            trade_count: strategy_summary.trade_count,
            realized_pnl: decimal_to_f64(strategy_realized_pnl),
            market_value: sum_market_value(&strategy_positions),
            positions: strategy_positions,
            investable_amount: investable_amount_row.map(|row| decimal_to_f64(row.amount_jpy)),
            unused_investable_amount,
        };

        Ok(ReadPortfolioResult {
            priced_at: prices.priced_at,
            account,
            strategy,
        })
    }
}

fn to_position_dtos(
    positions: Vec<core_application::trade::PositionSummary>,
    prices: &HashMap<String, Decimal>,
) -> Vec<PortfolioPositionDto> {
    positions
        .into_iter()
        .map(|p| to_position_dto(p, prices))
        .collect()
}

fn to_position_dto(
    p: core_application::trade::PositionSummary,
    prices: &HashMap<String, Decimal>,
) -> PortfolioPositionDto {
    let current_price = prices.get(&p.symbol).copied();
    let market_value = current_price.map(|price| p.qty * price);
    let unrealized_pnl = market_value.map(|mv| mv - p.cost_basis);
    PortfolioPositionDto {
        symbol: p.symbol,
        qty: decimal_to_f64(p.qty),
        avg_cost: decimal_to_f64(p.avg_cost),
        cost_basis: decimal_to_f64(p.cost_basis),
        realized_pnl: decimal_to_f64(p.realized_pnl),
        current_price: current_price.map(decimal_to_f64),
        market_value: market_value.map(decimal_to_f64),
        unrealized_pnl: unrealized_pnl.map(decimal_to_f64),
    }
}

fn sum_market_value(positions: &[PortfolioPositionDto]) -> f64 {
    positions.iter().filter_map(|p| p.market_value).sum()
}
