use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

use crate::bars::{BarsUseCaseError, BarsUseCases};

use super::{PerformanceSummary, PositionSummary, TradeUseCaseError, TradeUseCases};

#[derive(Debug, Error)]
pub enum TradeSummaryUseCaseError {
    #[error(transparent)]
    Trade(#[from] TradeUseCaseError),
    #[error(transparent)]
    Bars(#[from] BarsUseCaseError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValuedPositionSummary {
    pub symbol: String,
    pub qty: Decimal,
    pub avg_cost: Decimal,
    pub cost_basis: Decimal,
    pub realized_pnl: Decimal,
    pub current_price: Option<Decimal>,
    pub market_value: Option<Decimal>,
    pub unrealized_pnl: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValuedPerformanceSummary {
    pub strategy_id: Option<Uuid>,
    pub trade_count: i64,
    pub realized_pnl: Decimal,
    pub price_observed_at: Option<NaiveDate>,
    pub positions: Vec<ValuedPositionSummary>,
}

#[derive(Clone)]
pub struct TradeSummaryUseCases {
    trades: TradeUseCases,
    bars: BarsUseCases,
}

impl TradeSummaryUseCases {
    pub fn new(trades: TradeUseCases, bars: BarsUseCases) -> Self {
        Self { trades, bars }
    }

    pub async fn summary(
        &self,
        strategy_id: Option<Uuid>,
    ) -> Result<ValuedPerformanceSummary, TradeSummaryUseCaseError> {
        let summary = self.trades.summary(strategy_id).await?;
        let symbols = summary
            .positions
            .iter()
            .map(|position| position.symbol.clone())
            .collect::<Vec<_>>();
        let latest_prices = self.bars.find_latest_prices(&symbols).await?;

        Ok(with_market_values(summary, latest_prices))
    }
}

fn with_market_values(
    summary: PerformanceSummary,
    latest_prices: crate::bars::LatestPrices,
) -> ValuedPerformanceSummary {
    ValuedPerformanceSummary {
        strategy_id: summary.strategy_id,
        trade_count: summary.trade_count,
        realized_pnl: summary.realized_pnl,
        price_observed_at: latest_prices.priced_at,
        positions: summary
            .positions
            .into_iter()
            .map(|position| with_market_value(position, &latest_prices.prices))
            .collect(),
    }
}

fn with_market_value(
    position: PositionSummary,
    prices: &std::collections::HashMap<String, Decimal>,
) -> ValuedPositionSummary {
    let current_price = prices.get(&position.symbol).copied();
    let market_value = current_price.map(|price| position.qty * price);
    let unrealized_pnl = market_value.map(|value| value - position.cost_basis);

    ValuedPositionSummary {
        symbol: position.symbol,
        qty: position.qty,
        avg_cost: position.avg_cost,
        cost_basis: position.cost_basis,
        realized_pnl: position.realized_pnl,
        current_price,
        market_value,
        unrealized_pnl,
    }
}
