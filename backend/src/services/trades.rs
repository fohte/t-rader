//! 取引履歴の派生計算: FIFO ベースで実現損益・未決済ポジション・平均取得単価を出す。

use rust_decimal::Decimal;
use std::collections::HashMap;
use std::collections::VecDeque;

use crate::entities::trade;
use crate::models::{PerformanceSummary, PositionSummary};

/// FIFO Lot
#[derive(Debug, Clone)]
struct Lot {
    qty: Decimal,
    price: Decimal,
}

/// 取引の集合からポジションと実現損益を算出する。
///
/// 入力は同一戦略内・複数銘柄を想定。順序は (date, created_at) で並んでいる想定。
pub fn summarize(strategy_id: Option<uuid::Uuid>, trades: &[trade::Model]) -> PerformanceSummary {
    let mut lots_by_symbol: HashMap<String, VecDeque<Lot>> = HashMap::new();
    let mut realized_by_symbol: HashMap<String, Decimal> = HashMap::new();

    for t in trades {
        let lots = lots_by_symbol.entry(t.symbol.clone()).or_default();
        let realized = realized_by_symbol.entry(t.symbol.clone()).or_default();
        let net_price = t.price;

        match t.side.as_str() {
            "buy" => {
                lots.push_back(Lot {
                    qty: t.qty,
                    price: net_price,
                });
                // fee は実現損益を引く形で表現
                *realized -= t.fee;
            }
            "sell" => {
                let mut remaining = t.qty;
                while remaining > Decimal::ZERO {
                    let Some(front) = lots.front_mut() else {
                        // ロット不足時はコスト 0 として残量を実現損益に足す。
                        // CSV import 前の保有 (初期残高) や空売りでは値が過大になり得る
                        *realized += remaining * net_price;
                        break;
                    };
                    let take = remaining.min(front.qty);
                    *realized += take * (net_price - front.price);
                    front.qty -= take;
                    remaining -= take;
                    if front.qty == Decimal::ZERO {
                        lots.pop_front();
                    }
                }
                *realized -= t.fee;
            }
            _ => {
                // CHECK 制約があるため通常到達しないが、純粋関数経由 (テスト fixture 等)
                // で未知 side が紛れ込むと PnL が無音でズレるので warn 出力する
                tracing::warn!(
                    side = %t.side,
                    trade_id = %t.id,
                    "unknown trade side; skipped in summary",
                );
            }
        }
    }

    let mut positions: Vec<PositionSummary> = lots_by_symbol
        .into_iter()
        .filter_map(|(symbol, lots)| {
            let total_qty: Decimal = lots.iter().map(|l| l.qty).sum();
            if total_qty == Decimal::ZERO {
                return None;
            }
            let cost_basis: Decimal = lots.iter().map(|l| l.qty * l.price).sum();
            let avg_cost = if total_qty != Decimal::ZERO {
                cost_basis / total_qty
            } else {
                Decimal::ZERO
            };
            let realized_pnl = realized_by_symbol
                .get(&symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            Some(PositionSummary {
                symbol,
                qty: total_qty,
                avg_cost,
                cost_basis,
                realized_pnl,
            })
        })
        .collect();
    positions.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    let realized_pnl: Decimal = realized_by_symbol.values().copied().sum();

    PerformanceSummary {
        strategy_id,
        trade_count: trades.len() as i64,
        realized_pnl,
        positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rstest::rstest;
    use uuid::Uuid;

    fn trade(side: &str, qty: i64, price: i64, fee: i64) -> trade::Model {
        trade::Model {
            id: Uuid::new_v4(),
            strategy_id: Uuid::nil(),
            symbol: "7203".into(),
            side: side.into(),
            qty: Decimal::from(qty),
            price: Decimal::from(price),
            fee: Decimal::from(fee),
            date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("invalid date"),
            source: "manual".into(),
            note: None,
            created_at: chrono::Utc::now().fixed_offset(),
            updated_at: chrono::Utc::now().fixed_offset(),
        }
    }

    #[rstest]
    fn test_summarize_open_position() {
        let trades = vec![trade("buy", 100, 1000, 0)];
        let s = summarize(None, &trades);
        assert_eq!(s.positions.len(), 1);
        assert_eq!(s.positions[0].qty, Decimal::from(100));
        assert_eq!(s.positions[0].avg_cost, Decimal::from(1000));
        assert_eq!(s.realized_pnl, Decimal::ZERO);
    }

    #[rstest]
    fn test_summarize_fifo_realized() {
        let trades = vec![
            trade("buy", 100, 1000, 0),
            trade("buy", 100, 1200, 0),
            trade("sell", 150, 1300, 0),
        ];
        let s = summarize(None, &trades);
        // 1st lot 全消化: (1300-1000)*100 = 30000
        // 2nd lot 50 消化: (1300-1200)*50 = 5000
        assert_eq!(s.realized_pnl, Decimal::from(35000));
        assert_eq!(s.positions.len(), 1);
        assert_eq!(s.positions[0].qty, Decimal::from(50));
        assert_eq!(s.positions[0].avg_cost, Decimal::from(1200));
    }

    #[rstest]
    fn test_summarize_fully_closed() {
        let trades = vec![trade("buy", 100, 1000, 10), trade("sell", 100, 1100, 20)];
        let s = summarize(None, &trades);
        // realized: 100*(1100-1000) - 10 - 20 = 10000 - 30 = 9970
        assert_eq!(s.realized_pnl, Decimal::from(9970));
        assert!(s.positions.is_empty());
    }
}
