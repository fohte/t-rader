use std::collections::{HashMap, VecDeque};

use chrono::NaiveDate;
use core_domain::bar::Bar;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::{
    PaperAccount, PaperOrderSide, PaperOrderWithResult, PaperTradeUseCaseError,
    adjustment::{map_split_adjustment_error, split_cursor_from_bars},
    types::PaperOrderResult,
};
use crate::split_adjustment::adjust_lot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OpenLot {
    pub qty: i64,
    pub fill_price: Decimal,
    pub fill_date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClosedLotMatch {
    pub qty: i64,
    pub buy_price: Decimal,
    pub buy_date: NaiveDate,
    pub sell_price: Decimal,
    pub sell_date: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClosedTrade {
    pub matches: Vec<ClosedLotMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PaperTradeLedger {
    pub cash_jpy: Decimal,
    pub lots_by_stock: HashMap<String, VecDeque<OpenLot>>,
    pub closed_trades: Vec<ClosedTrade>,
}

impl PaperTradeLedger {
    pub fn open_holdings(&self) -> impl Iterator<Item = (&str, i64, Decimal)> + '_ {
        self.lots_by_stock.iter().filter_map(|(stock_id, lots)| {
            let qty = lots.iter().map(|lot| lot.qty).sum::<i64>();
            (qty > 0).then(|| {
                let cost_basis = lots.iter().fold(Decimal::ZERO, |total, lot| {
                    total + Decimal::from(lot.qty) * lot.fill_price
                });
                (stock_id.as_str(), qty, cost_basis)
            })
        })
    }

    fn apply_split(
        &mut self,
        instrument_id: &str,
        factor: Decimal,
    ) -> Result<(), PaperTradeUseCaseError> {
        let Some(lots) = self.lots_by_stock.get_mut(instrument_id) else {
            return Ok(());
        };
        for lot in lots.iter_mut() {
            let (quantity, price) = adjust_lot(Decimal::from(lot.qty), lot.fill_price, factor)
                .map_err(map_split_adjustment_error)?;
            lot.qty = quantity.round_dp(0).to_i64().ok_or_else(|| {
                PaperTradeUseCaseError::Validation("split-adjusted quantity overflowed".into())
            })?;
            lot.fill_price = price;
        }
        lots.retain(|lot| lot.qty > 0);
        Ok(())
    }
}

pub(super) fn replay_fills(
    account: &PaperAccount,
    orders: &[PaperOrderWithResult],
    split_bars: &[Bar],
) -> Result<PaperTradeLedger, PaperTradeUseCaseError> {
    let mut events = orders
        .iter()
        .filter_map(|item| match item.result.as_ref() {
            Some(PaperOrderResult::Filled {
                fill_date,
                fill_price,
                ..
            }) => Some((item, *fill_date, *fill_price)),
            Some(PaperOrderResult::Rejected { .. }) | None => None,
        })
        .collect::<Vec<_>>();
    events.sort_by(|(left, left_date, _), (right, right_date, _)| {
        left_date
            .cmp(right_date)
            .then_with(|| left.order.ordered_at.cmp(&right.order.ordered_at))
            .then_with(|| left.order.id.cmp(&right.order.id))
    });
    let mut split_cursor = split_cursor_from_bars(split_bars.iter())?;

    let mut ledger = PaperTradeLedger {
        cash_jpy: account.initial_cash_jpy,
        lots_by_stock: HashMap::new(),
        closed_trades: Vec::new(),
    };

    for (item, fill_date, fill_price) in events {
        split_cursor.apply_through(fill_date, |adjustment| {
            ledger.apply_split(&adjustment.instrument_id, adjustment.factor)
        })?;
        let quantity = Decimal::from(item.order.qty);
        let amount = fill_price.checked_mul(quantity).ok_or_else(|| {
            PaperTradeUseCaseError::Validation("paper order value overflowed".into())
        })?;
        match item.order.side {
            PaperOrderSide::Buy => {
                ledger.cash_jpy = ledger.cash_jpy.checked_sub(amount).ok_or_else(|| {
                    PaperTradeUseCaseError::Validation("paper cash balance overflowed".into())
                })?;
                ledger
                    .lots_by_stock
                    .entry(item.order.stock_id.clone())
                    .or_default()
                    .push_back(OpenLot {
                        qty: item.order.qty,
                        fill_price,
                        fill_date,
                    });
            }
            PaperOrderSide::Sell => {
                ledger.cash_jpy = ledger.cash_jpy.checked_add(amount).ok_or_else(|| {
                    PaperTradeUseCaseError::Validation("paper cash balance overflowed".into())
                })?;
                let lots = ledger
                    .lots_by_stock
                    .get_mut(&item.order.stock_id)
                    .ok_or_else(|| {
                        PaperTradeUseCaseError::Validation(
                            "filled sale has no matching purchase lots".into(),
                        )
                    })?;
                let mut remaining = item.order.qty;
                let mut matches = Vec::new();
                while remaining > 0 {
                    let Some(front) = lots.front_mut() else {
                        return Err(PaperTradeUseCaseError::Validation(
                            "filled sale exceeds available purchase lots".into(),
                        ));
                    };
                    let matched_qty = remaining.min(front.qty);
                    matches.push(ClosedLotMatch {
                        qty: matched_qty,
                        buy_price: front.fill_price,
                        buy_date: front.fill_date,
                        sell_price: fill_price,
                        sell_date: fill_date,
                    });
                    front.qty -= matched_qty;
                    remaining -= matched_qty;
                    if front.qty == 0 {
                        lots.pop_front();
                    }
                }
                ledger.closed_trades.push(ClosedTrade { matches });
            }
        }
    }

    split_cursor.apply_remaining(|adjustment| {
        ledger.apply_split(&adjustment.instrument_id, adjustment.factor)
    })?;

    Ok(ledger)
}
