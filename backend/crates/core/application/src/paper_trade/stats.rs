use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, NaiveTime};
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::bars::BarsByInstrumentsQuery;

use super::{
    PaperAccount, PaperOrderWithResult, PaperTradeAccountStats, PaperTradeUseCaseError,
    PaperTradeUseCases,
    ledger::{ClosedTrade, PaperTradeLedger, replay_fills},
    portfolio::position,
};

impl PaperTradeUseCases {
    pub async fn stats(&self) -> Result<Vec<PaperTradeAccountStats>, PaperTradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let mut accounts = self.repository.list_accounts(&transaction).await?;
        accounts.sort_by_key(|account| account.id);
        let orders = self
            .repository
            .list_orders_with_results(&transaction, None)
            .await?;
        self.unit_of_work.commit(transaction).await?;
        let orders_by_account = group_orders_by_account(orders);
        let ledgers = accounts
            .iter()
            .map(|account| {
                let ledger = replay_fills(
                    account,
                    orders_by_account
                        .get(&account.id)
                        .map_or(&[], Vec::as_slice),
                )?;
                Ok((account.id, ledger))
            })
            .collect::<Result<HashMap<_, _>, PaperTradeUseCaseError>>()?;

        let held_stock_ids = ledgers
            .values()
            .flat_map(|ledger| ledger.lots_by_stock.iter())
            .filter(|(_, lots)| lots.iter().map(|lot| lot.qty).sum::<i64>() > 0)
            .map(|(stock_id, _)| stock_id.clone())
            .collect::<HashSet<_>>();
        let mut latest_bars = HashMap::<String, Bar>::new();
        for stock_id in held_stock_ids {
            let bar = self
                .bars
                .find_latest_bar(&stock_id, "1d")
                .await?
                .ok_or_else(|| {
                    PaperTradeUseCaseError::LatestDailyBarUnavailable(stock_id.clone())
                })?;
            latest_bars.insert(stock_id, bar);
        }

        let benchmark_ids = accounts
            .iter()
            .filter_map(|account| account.benchmark_stock_id.clone())
            .collect::<HashSet<_>>();
        let benchmark_bars = if benchmark_ids.is_empty() {
            HashMap::new()
        } else {
            let from = accounts
                .iter()
                .map(|account| account.started_on)
                .min()
                .unwrap_or_default();
            let bars = self
                .bars
                .find_bars_by_instruments(BarsByInstrumentsQuery {
                    instrument_ids: benchmark_ids.into_iter().collect(),
                    timeframe: "1d".to_string(),
                    from: Some(utc_midnight(from)),
                    to: None,
                })
                .await?;
            index_bars_by_instrument_and_date(bars)
        };

        accounts
            .into_iter()
            .map(|account| {
                let ledger = ledgers
                    .get(&account.id)
                    .ok_or(PaperTradeUseCaseError::AccountNotFound(account.id))?;
                let benchmark = account
                    .benchmark_stock_id
                    .as_ref()
                    .and_then(|stock_id| benchmark_bars.get(stock_id));
                summarize_account(&account, ledger, &latest_bars, benchmark)
            })
            .collect()
    }
}

fn group_orders_by_account(
    orders: Vec<PaperOrderWithResult>,
) -> HashMap<Uuid, Vec<PaperOrderWithResult>> {
    let mut grouped = HashMap::<Uuid, Vec<PaperOrderWithResult>>::new();
    for item in orders {
        grouped.entry(item.order.account_id).or_default().push(item);
    }
    grouped
}

fn summarize_account(
    account: &PaperAccount,
    ledger: &PaperTradeLedger,
    latest_bars: &HashMap<String, Bar>,
    benchmark_bars: Option<&HashMap<NaiveDate, Bar>>,
) -> Result<PaperTradeAccountStats, PaperTradeUseCaseError> {
    let mut market_value_jpy = Decimal::ZERO;
    let mut unrealized_pnl_jpy = Decimal::ZERO;
    let mut as_of = account.started_on;
    for (stock_id, lots) in &ledger.lots_by_stock {
        let qty = lots.iter().map(|lot| lot.qty).sum::<i64>();
        if qty == 0 {
            continue;
        }
        let cost_basis = lots.iter().fold(Decimal::ZERO, |total, lot| {
            total + Decimal::from(lot.qty) * lot.fill_price
        });
        let latest_bar = latest_bars
            .get(stock_id)
            .ok_or_else(|| PaperTradeUseCaseError::LatestDailyBarUnavailable(stock_id.clone()))?;
        let position = position(stock_id.clone(), qty, cost_basis, latest_bar);
        market_value_jpy += position.market_value_jpy;
        unrealized_pnl_jpy += position.unrealized_pnl_jpy;
        as_of = as_of.max(latest_bar.timestamp.date_naive());
    }

    let total_assets_jpy = ledger.cash_jpy + market_value_jpy;
    let return_since_start = decimal_ratio(total_assets_jpy, account.initial_cash_jpy)
        .map(|ratio| ratio - Decimal::ONE)
        .ok_or_else(|| {
            PaperTradeUseCaseError::Validation("paper account return is undefined".into())
        })?;
    let benchmark_return =
        benchmark_bars.and_then(|bars| benchmark_return(account.started_on, bars));
    let mut evaluated_excess_returns = Vec::new();
    if let Some(bars) = benchmark_bars {
        evaluated_excess_returns.extend(
            ledger
                .closed_trades
                .iter()
                .filter_map(|trade| excess_return(trade, bars)),
        );
        if let Some(latest_benchmark_date) = bars.keys().max().copied() {
            as_of = as_of.max(latest_benchmark_date);
        }
    }

    let wins = evaluated_excess_returns
        .iter()
        .copied()
        .filter(|value| *value > Decimal::ZERO)
        .collect::<Vec<_>>();
    let losses = evaluated_excess_returns
        .iter()
        .copied()
        .filter(|value| *value < Decimal::ZERO)
        .collect::<Vec<_>>();
    let win_rate = (!evaluated_excess_returns.is_empty())
        .then(|| Decimal::from(wins.len()) / Decimal::from(evaluated_excess_returns.len()));

    Ok(PaperTradeAccountStats {
        account: account.clone(),
        as_of,
        total_assets_jpy,
        return_since_start,
        benchmark_return,
        closed_trade_count: ledger.closed_trades.len(),
        win_rate,
        average_win_excess_return: average(&wins),
        average_loss_excess_return: average(&losses),
        unrealized_pnl_jpy,
    })
}

fn benchmark_return(started_on: NaiveDate, bars: &HashMap<NaiveDate, Bar>) -> Option<Decimal> {
    let starting_bar = bars
        .iter()
        .filter(|(date, _)| **date >= started_on)
        .min_by_key(|(date, _)| *date)
        .map(|(_, bar)| bar)?;
    let ending_bar = bars
        .iter()
        .max_by_key(|(date, _)| *date)
        .map(|(_, bar)| bar)?;
    decimal_ratio(ending_bar.close, starting_bar.open).map(|ratio| ratio - Decimal::ONE)
}

fn excess_return(trade: &ClosedTrade, bars: &HashMap<NaiveDate, Bar>) -> Option<Decimal> {
    let total_qty = trade.matches.iter().map(|matched| matched.qty).sum::<i64>();
    if total_qty == 0 {
        return None;
    }
    let weighted_excess = trade
        .matches
        .iter()
        .try_fold(Decimal::ZERO, |total, matched| {
            let buy_benchmark = bars.get(&matched.buy_date)?.open;
            let sell_benchmark = bars.get(&matched.sell_date)?.open;
            let stock_return = decimal_ratio(matched.sell_price, matched.buy_price)? - Decimal::ONE;
            let benchmark_return = decimal_ratio(sell_benchmark, buy_benchmark)? - Decimal::ONE;
            Some(total + Decimal::from(matched.qty) * (stock_return - benchmark_return))
        })?;
    decimal_ratio(weighted_excess, Decimal::from(total_qty))
}

fn average(values: &[Decimal]) -> Option<Decimal> {
    if values.is_empty() {
        return None;
    }
    let total = values.iter().copied().sum::<Decimal>();
    decimal_ratio(total, Decimal::from(values.len()))
}

fn decimal_ratio(numerator: Decimal, denominator: Decimal) -> Option<Decimal> {
    numerator.checked_div(denominator)
}

fn index_bars_by_instrument_and_date(bars: Vec<Bar>) -> HashMap<String, HashMap<NaiveDate, Bar>> {
    let mut indexed = HashMap::<String, HashMap<NaiveDate, Bar>>::new();
    for bar in bars {
        indexed
            .entry(bar.instrument_id.clone())
            .or_default()
            .insert(bar.timestamp.date_naive(), bar);
    }
    indexed
}

fn utc_midnight(date: NaiveDate) -> chrono::DateTime<chrono::FixedOffset> {
    date.and_time(NaiveTime::MIN).and_utc().fixed_offset()
}

#[cfg(all(test, feature = "test-support"))]
mod tests;
