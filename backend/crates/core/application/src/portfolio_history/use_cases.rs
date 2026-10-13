use std::collections::HashMap;

use chrono::NaiveDate;
use core_domain::bar::Bar;
use rust_decimal::Decimal;

use crate::bars::{BarsByInstrumentsQuery, SharedBarsRepository};
use crate::split_adjustment::SplitCursor;
use crate::strategy::{InvestableAmount, SharedStrategyRepository};
use crate::trade::{
    SharedTradeRepository, TradeListItem, TradeOrder, TradeQuery, split_cursor_for_rows, summarize,
};

use super::error::PortfolioHistoryUseCaseError;
use super::types::{PortfolioHistoryPoint, PortfolioHistoryQuery};

const MAX_HISTORY_DAYS: i64 = 3_660;

#[derive(Clone)]
pub struct PortfolioHistoryUseCases {
    trades: SharedTradeRepository,
    bars: SharedBarsRepository,
    strategies: SharedStrategyRepository,
}

impl PortfolioHistoryUseCases {
    pub fn new(
        trades: SharedTradeRepository,
        bars: SharedBarsRepository,
        strategies: SharedStrategyRepository,
    ) -> Self {
        Self {
            trades,
            bars,
            strategies,
        }
    }

    pub async fn history(
        &self,
        query: PortfolioHistoryQuery,
    ) -> Result<Vec<PortfolioHistoryPoint>, PortfolioHistoryUseCaseError> {
        if query.from > query.to
            || query.to.signed_duration_since(query.from).num_days() >= MAX_HISTORY_DAYS
        {
            return Err(PortfolioHistoryUseCaseError::InvalidDateRange);
        }

        let trades = self
            .trades
            .list(TradeQuery {
                strategy_id: None,
                symbol: None,
                date_from: None,
                limit: None,
                order: TradeOrder::DateAscending,
                include_note_count: false,
                include_note_references: false,
            })
            .await?
            .into_iter()
            .filter(|row| row.trade.date <= query.to)
            .collect::<Vec<_>>();
        let symbols = trade_symbols(&trades);
        let split_cursor = split_cursor_for_rows(self.bars.as_ref(), &trades).await?;
        let bars = self.bars_until(&symbols, query.to).await?;
        let amounts = self.strategies.find_investable_amount_history().await?;

        build_history(query, &trades, split_cursor, &bars, &amounts)
    }

    async fn bars_until(
        &self,
        symbols: &[String],
        to: NaiveDate,
    ) -> Result<Vec<Bar>, PortfolioHistoryUseCaseError> {
        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        let to = to
            .and_hms_nano_opt(23, 59, 59, 999_999_999)
            .map(|datetime| datetime.and_utc().fixed_offset())
            .ok_or_else(|| {
                PortfolioHistoryUseCaseError::Calculation(
                    "could not construct end-of-day timestamp".into(),
                )
            })?;
        let mut bars = self
            .bars
            .find_bars_by_instruments(BarsByInstrumentsQuery {
                instrument_ids: symbols.to_vec(),
                timeframe: "1d".into(),
                from: None,
                to: Some(to),
            })
            .await?;
        bars.sort_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then_with(|| left.instrument_id.cmp(&right.instrument_id))
        });
        Ok(bars)
    }
}

fn trade_symbols(trades: &[TradeListItem]) -> Vec<String> {
    let mut symbols = trades
        .iter()
        .map(|row| row.trade.symbol.clone())
        .collect::<Vec<_>>();
    symbols.sort();
    symbols.dedup();
    symbols
}

fn build_history(
    query: PortfolioHistoryQuery,
    trades: &[TradeListItem],
    split_cursor: SplitCursor,
    bars: &[Bar],
    amounts: &[InvestableAmount],
) -> Result<Vec<PortfolioHistoryPoint>, PortfolioHistoryUseCaseError> {
    let mut latest_prices = HashMap::<String, Decimal>::new();
    let mut next_bar = 0;
    let mut points = Vec::new();
    let mut date = query.from;
    let mut summary = summarize(None, &trades[..0], split_cursor.clone())?;
    let mut summarized_trade_count = 0;
    loop {
        while bars
            .get(next_bar)
            .is_some_and(|bar| bar.timestamp.date_naive() <= date)
        {
            let bar = &bars[next_bar];
            latest_prices.insert(bar.instrument_id.clone(), bar.close);
            next_bar += 1;
        }

        let trade_count = trades.partition_point(|row| row.trade.date <= date);
        if trade_count != summarized_trade_count {
            // 保存済み日足は将来の分割を反映した基準なので、過去日の保有にも同じ係数を適用する。
            summary = summarize(None, &trades[..trade_count], split_cursor.clone())?;
            summarized_trade_count = trade_count;
        }
        let investable_amount = investable_amount_at(amounts, date);
        let cost_basis: Decimal = summary
            .positions
            .iter()
            .map(|position| position.cost_basis)
            .sum();
        let stock_market_value: Decimal = summary
            .positions
            .iter()
            .map(|position| {
                latest_prices
                    .get(&position.symbol)
                    .map_or(position.cost_basis, |price| position.qty * price)
            })
            .sum();
        let cash = investable_amount + summary.realized_pnl - cost_basis;

        points.push(PortfolioHistoryPoint {
            date,
            total_assets: cash + stock_market_value,
            stock_market_value,
            cash,
            investable_amount,
        });

        if date == query.to {
            break;
        }
        date = date
            .succ_opt()
            .ok_or(PortfolioHistoryUseCaseError::InvalidDateRange)?;
    }

    Ok(points)
}

fn investable_amount_at(amounts: &[InvestableAmount], date: NaiveDate) -> Decimal {
    let mut total = Decimal::ZERO;
    let mut start = 0;
    while start < amounts.len() {
        let strategy_id = amounts[start].strategy_id;
        let mut end = start + 1;
        while end < amounts.len() && amounts[end].strategy_id == strategy_id {
            end += 1;
        }

        let history = &amounts[start..end];
        let current = history
            .iter()
            .take_while(|amount| amount.effective_at.date_naive() <= date)
            .last()
            .or_else(|| history.first());
        total += current.map_or(Decimal::ZERO, |amount| amount.amount_jpy);
        start = end;
    }
    total
}
