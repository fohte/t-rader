use std::collections::{HashMap, VecDeque};

use rust_decimal::Decimal;
use uuid::Uuid;

use crate::split_adjustment::{SplitAdjustment, SplitAdjustmentError, SplitCursor, adjust_lot};

use super::error::TradeUseCaseError;
use super::types::{
    MatchedBuy, PerformanceSummary, PositionSummary, Trade, TradeDetail, TradeListItem,
};

#[derive(Debug, Clone)]
struct Lot {
    trade_id: Uuid,
    date: chrono::NaiveDate,
    qty: Decimal,
    price: Decimal,
    remaining_trade_qty: Decimal,
    execution_price: Decimal,
}

fn push_buy_lot(lots: &mut VecDeque<Lot>, trade: &Trade) {
    lots.push_back(Lot {
        trade_id: trade.id,
        date: trade.date,
        qty: trade.qty,
        price: trade.price,
        remaining_trade_qty: trade.qty,
        execution_price: trade.price,
    });
}

pub(super) fn split_adjustment_error(
    error: SplitAdjustmentError,
    adjustment: &SplitAdjustment,
) -> TradeUseCaseError {
    TradeUseCaseError::FifoCalculation(format!(
        "split adjustment for {} on {} failed: {error}",
        adjustment.instrument_id, adjustment.date
    ))
}

fn adjust_lots(
    lots: &mut VecDeque<Lot>,
    adjustment: &SplitAdjustment,
) -> Result<(), TradeUseCaseError> {
    for lot in lots.iter_mut() {
        (lot.qty, lot.price) = adjust_lot(lot.qty, lot.price, adjustment.factor)
            .map_err(|error| split_adjustment_error(error, adjustment))?;
    }
    lots.retain(|lot| lot.qty > Decimal::ZERO);
    Ok(())
}

fn apply_split_adjustment(
    lots_by_symbol: &mut HashMap<String, VecDeque<Lot>>,
    adjustment: &SplitAdjustment,
) -> Result<(), TradeUseCaseError> {
    if let Some(lots) = lots_by_symbol.get_mut(&adjustment.instrument_id) {
        adjust_lots(lots, adjustment)?;
    }
    Ok(())
}

fn consume_sell_lots(
    lots: &mut VecDeque<Lot>,
    trade: &Trade,
    mut on_match: impl FnMut(&Lot, Decimal),
) -> Decimal {
    let mut remaining = trade.qty;
    let mut realized_pnl = Decimal::ZERO;
    while remaining > Decimal::ZERO {
        let Some(front) = lots.front_mut() else {
            realized_pnl += remaining * trade.price;
            break;
        };
        let taken = remaining.min(front.qty);
        let trade_basis_taken = if taken == front.qty {
            front.remaining_trade_qty
        } else {
            front.remaining_trade_qty * taken / front.qty
        };
        on_match(front, trade_basis_taken);
        realized_pnl += taken * (trade.price - front.price);
        front.qty -= taken;
        front.remaining_trade_qty -= trade_basis_taken;
        remaining -= taken;
        if front.qty == Decimal::ZERO {
            lots.pop_front();
        }
    }
    realized_pnl - trade.fee
}

pub(super) fn calculate_trade_detail(
    trade: Trade,
    rows: &[TradeListItem],
    mut split_cursor: SplitCursor,
) -> Result<TradeDetail, TradeUseCaseError> {
    let mut lots = VecDeque::new();
    let mut realized_pnl = None;
    let mut matched_buys = None;
    let mut target_buy_seen = false;

    for row in rows {
        split_cursor.apply_through(row.trade.date, |adjustment| {
            adjust_lots(&mut lots, adjustment)
        })?;
        match row.trade.side.as_str() {
            "buy" => {
                push_buy_lot(&mut lots, &row.trade);
                if row.trade.id == trade.id {
                    target_buy_seen = true;
                }
            }
            "sell" => {
                let is_target = row.trade.id == trade.id;
                let mut sale_matches = Vec::new();
                let sale_pnl = consume_sell_lots(&mut lots, &row.trade, |lot, trade_basis_qty| {
                    if is_target {
                        sale_matches.push(MatchedBuy {
                            date: lot.date,
                            qty: trade_basis_qty,
                            price: lot.execution_price,
                        });
                    }
                });
                if is_target {
                    realized_pnl = Some(sale_pnl);
                    matched_buys = Some(sale_matches);
                }
            }
            _ => {}
        }
    }

    split_cursor.apply_remaining(|adjustment| adjust_lots(&mut lots, adjustment))?;

    let remaining_qty = (trade.side == "buy").then(|| {
        if target_buy_seen {
            lots.iter()
                .find(|lot| lot.trade_id == trade.id)
                .map_or(Decimal::ZERO, |lot| lot.remaining_trade_qty)
        } else {
            trade.qty
        }
    });
    if trade.side == "sell" && realized_pnl.is_none() {
        realized_pnl = Some(-trade.fee);
        matched_buys = Some(Vec::new());
    }

    Ok(TradeDetail {
        trade,
        realized_pnl,
        matched_buys,
        remaining_qty,
    })
}

pub(crate) fn summarize(
    strategy_id: Option<Uuid>,
    rows: &[TradeListItem],
    mut split_cursor: SplitCursor,
) -> Result<PerformanceSummary, TradeUseCaseError> {
    let mut lots_by_symbol: HashMap<String, VecDeque<Lot>> = HashMap::new();
    let mut realized_by_symbol: HashMap<String, Decimal> = HashMap::new();

    for row in rows {
        let trade = &row.trade;
        split_cursor.apply_through(trade.date, |adjustment| {
            apply_split_adjustment(&mut lots_by_symbol, adjustment)
        })?;
        let lots = lots_by_symbol.entry(trade.symbol.clone()).or_default();
        let realized = realized_by_symbol.entry(trade.symbol.clone()).or_default();
        match trade.side.as_str() {
            "buy" => {
                push_buy_lot(lots, trade);
                *realized -= trade.fee;
            }
            "sell" => {
                *realized += consume_sell_lots(lots, trade, |_, _| {});
            }
            _ => tracing::warn!(
                side = %trade.side,
                trade_id = %trade.id,
                "unknown trade side; skipped in summary",
            ),
        }
    }

    split_cursor
        .apply_remaining(|adjustment| apply_split_adjustment(&mut lots_by_symbol, adjustment))?;

    let mut positions: Vec<PositionSummary> = lots_by_symbol
        .into_iter()
        .filter_map(|(symbol, lots)| {
            let qty: Decimal = lots.iter().map(|lot| lot.qty).sum();
            if qty == Decimal::ZERO {
                return None;
            }
            let cost_basis: Decimal = lots.iter().map(|lot| lot.qty * lot.price).sum();
            Some(PositionSummary {
                realized_pnl: realized_by_symbol
                    .get(&symbol)
                    .copied()
                    .unwrap_or(Decimal::ZERO),
                symbol,
                qty,
                avg_cost: cost_basis / qty,
                cost_basis,
            })
        })
        .collect();
    positions.sort_by(|left, right| left.symbol.cmp(&right.symbol));

    Ok(PerformanceSummary {
        strategy_id,
        trade_count: rows.len() as i64,
        realized_pnl: realized_by_symbol.values().copied().sum(),
        positions,
    })
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;

    use crate::split_adjustment::SplitCursor;
    use crate::trade::types::{PerformanceSummary, PositionSummary, Trade, TradeListItem};

    use super::summarize;

    fn trade_row(side: &str, qty: i64, price: i64, fee: i64) -> TradeListItem {
        TradeListItem {
            trade: Trade {
                id: uuid::Uuid::nil(),
                strategy_id: uuid::Uuid::nil(),
                symbol: "FICTIONAL-SYMBOL".into(),
                side: side.into(),
                qty: rust_decimal::Decimal::from(qty),
                price: rust_decimal::Decimal::from(price),
                fee: rust_decimal::Decimal::from(fee),
                date: NaiveDate::MIN,
                source: "manual".into(),
                note: None,
                created_at: chrono::Utc::now().fixed_offset(),
                updated_at: chrono::Utc::now().fixed_offset(),
            },
            note_count: 0,
            note_references: Vec::new(),
        }
    }

    #[rstest]
    #[case::open_position(
        vec![trade_row("buy", 100, 1000, 0)],
        PerformanceSummary {
            strategy_id: None,
            trade_count: 1,
            realized_pnl: rust_decimal::Decimal::ZERO,
            positions: vec![PositionSummary {
                symbol: "FICTIONAL-SYMBOL".into(),
                qty: rust_decimal::Decimal::from(100),
                avg_cost: rust_decimal::Decimal::from(1000),
                cost_basis: rust_decimal::Decimal::from(100000),
                realized_pnl: rust_decimal::Decimal::ZERO,
            }],
        },
    )]
    #[case::fifo_partial_sale(
        vec![
            trade_row("buy", 100, 1000, 0),
            trade_row("buy", 100, 1200, 0),
            trade_row("sell", 150, 1300, 0),
        ],
        PerformanceSummary {
            strategy_id: None,
            trade_count: 3,
            realized_pnl: rust_decimal::Decimal::from(35000),
            positions: vec![PositionSummary {
                symbol: "FICTIONAL-SYMBOL".into(),
                qty: rust_decimal::Decimal::from(50),
                avg_cost: rust_decimal::Decimal::from(1200),
                cost_basis: rust_decimal::Decimal::from(60000),
                realized_pnl: rust_decimal::Decimal::from(35000),
            }],
        },
    )]
    #[case::fully_closed_with_fees(
        vec![
            trade_row("buy", 100, 1000, 10),
            trade_row("sell", 100, 1100, 20),
        ],
        PerformanceSummary {
            strategy_id: None,
            trade_count: 2,
            realized_pnl: rust_decimal::Decimal::from(9970),
            positions: vec![],
        },
    )]
    fn summarize_preserves_fifo_results(
        #[case] rows: Vec<TradeListItem>,
        #[case] expected: PerformanceSummary,
    ) {
        let split_cursor = SplitCursor::new([]).expect("empty split cursor succeeds");
        assert_eq!(
            summarize(None, &rows, split_cursor).expect("summary succeeds"),
            expected,
        );
    }
}
