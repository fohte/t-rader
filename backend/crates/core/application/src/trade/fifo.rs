use std::collections::{HashMap, VecDeque};

use rust_decimal::Decimal;
use uuid::Uuid;

use super::types::{
    MatchedBuy, PerformanceSummary, PositionSummary, Trade, TradeDetail, TradeListItem,
};

#[derive(Debug, Clone)]
struct Lot {
    trade_id: Uuid,
    date: chrono::NaiveDate,
    qty: Decimal,
    price: Decimal,
}

fn push_buy_lot(lots: &mut VecDeque<Lot>, trade: &Trade) {
    lots.push_back(Lot {
        trade_id: trade.id,
        date: trade.date,
        qty: trade.qty,
        price: trade.price,
    });
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
        on_match(front, taken);
        realized_pnl += taken * (trade.price - front.price);
        front.qty -= taken;
        remaining -= taken;
        if front.qty == Decimal::ZERO {
            lots.pop_front();
        }
    }
    realized_pnl - trade.fee
}

pub(super) fn calculate_trade_detail(trade: Trade, rows: &[TradeListItem]) -> TradeDetail {
    let mut lots = VecDeque::new();
    let mut realized_pnl = None;
    let mut matched_buys = None;
    let mut target_buy_remaining = None;

    for row in rows {
        match row.trade.side.as_str() {
            "buy" => {
                push_buy_lot(&mut lots, &row.trade);
                if row.trade.id == trade.id {
                    target_buy_remaining = Some(row.trade.qty);
                }
            }
            "sell" => {
                let is_target = row.trade.id == trade.id;
                let mut sale_matches = Vec::new();
                let sale_pnl = consume_sell_lots(&mut lots, &row.trade, |lot, qty| {
                    if lot.trade_id == trade.id {
                        target_buy_remaining = Some(lot.qty - qty);
                    }
                    if is_target {
                        sale_matches.push(MatchedBuy {
                            date: lot.date,
                            qty,
                            price: lot.price,
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

    let remaining_qty = (trade.side == "buy").then(|| target_buy_remaining.unwrap_or(trade.qty));
    if trade.side == "sell" && realized_pnl.is_none() {
        realized_pnl = Some(-trade.fee);
        matched_buys = Some(Vec::new());
    }

    TradeDetail {
        trade,
        realized_pnl,
        matched_buys,
        remaining_qty,
    }
}

pub(super) fn summarize(strategy_id: Option<Uuid>, rows: &[TradeListItem]) -> PerformanceSummary {
    let mut lots_by_symbol: HashMap<String, VecDeque<Lot>> = HashMap::new();
    let mut realized_by_symbol: HashMap<String, Decimal> = HashMap::new();

    for row in rows {
        let trade = &row.trade;
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

    PerformanceSummary {
        strategy_id,
        trade_count: rows.len() as i64,
        realized_pnl: realized_by_symbol.values().copied().sum(),
        positions,
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use chrono::NaiveDate;
    use rstest::rstest;

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
        assert_eq!(summarize(None, &rows), expected);
    }
}
