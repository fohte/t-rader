use std::collections::{HashMap, VecDeque};

use chrono::Utc;
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, TargetKind};
use crate::strategy_existence::SharedStrategyExistence;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::TradeUseCaseError;
use super::repository::SharedTradeRepository;
use super::types::{
    CreateTradeCommand, NewTrade, PerformanceSummary, PositionSummary, Trade, TradeListItem,
    TradeOrder, TradeQuery, TradeUpdate, TradeUpdateCommand,
};

const ALLOWED_SIDE: [&str; 2] = ["buy", "sell"];
const ALLOWED_SOURCE: [&str; 3] = ["manual", "csv", "api"];

#[derive(Clone)]
pub struct TradeUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedTradeRepository,
    pub(super) strategy_existence: SharedStrategyExistence,
    pub(super) change_history: crate::change_history::SharedChangeHistoryPort,
}

impl TradeUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedTradeRepository,
        strategy_existence: SharedStrategyExistence,
        change_history: crate::change_history::SharedChangeHistoryPort,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            strategy_existence,
            change_history,
        }
    }

    pub async fn list(&self, query: TradeQuery) -> Result<Vec<TradeListItem>, TradeUseCaseError> {
        self.repository.list(query).await.map_err(Into::into)
    }

    pub async fn get(&self, id: Uuid) -> Result<Trade, TradeUseCaseError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(TradeUseCaseError::NotFound(id))
    }

    pub async fn create(&self, command: CreateTradeCommand) -> Result<Trade, TradeUseCaseError> {
        let symbol = command.symbol.trim().to_string();
        if symbol.is_empty() {
            return Err(TradeUseCaseError::Validation(
                "symbol must not be empty".into(),
            ));
        }
        validate_side(&command.side)?;
        validate_source(&command.source)?;
        if command.qty <= Decimal::ZERO {
            return Err(TradeUseCaseError::Validation("qty must be positive".into()));
        }
        if command.price < Decimal::ZERO {
            return Err(TradeUseCaseError::Validation(
                "price must be non-negative".into(),
            ));
        }

        let transaction = self.unit_of_work.begin().await?;
        self.ensure_strategy_exists(&transaction, command.strategy_id)
            .await?;
        let id = Uuid::new_v4();
        let trade = self
            .repository
            .insert(
                &transaction,
                NewTrade {
                    id,
                    strategy_id: command.strategy_id,
                    symbol: symbol.clone(),
                    side: command.side.clone(),
                    qty: command.qty,
                    price: command.price,
                    fee: command.fee.unwrap_or(Decimal::ZERO),
                    date: command.date,
                    source: command.source,
                    note: command.note,
                },
            )
            .await?;
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: Actor::Human,
                    target_kind: TargetKind::Trade,
                    target_id: id,
                    op: Op::Create,
                    diff: json!({
                        "strategy_id": command.strategy_id,
                        "symbol": symbol,
                        "side": command.side,
                        "qty": command.qty,
                        "price": command.price,
                    }),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(trade)
    }

    pub async fn update(
        &self,
        id: Uuid,
        command: TradeUpdateCommand,
    ) -> Result<Trade, TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        let current = self
            .repository
            .find_by_id_in_transaction(&transaction, id)
            .await?
            .ok_or(TradeUseCaseError::NotFound(id))?;
        let mut next = current.clone();
        let mut diff = Map::new();

        if let Some(value) = command.strategy_id {
            self.ensure_strategy_exists(&transaction, value).await?;
            diff.insert(
                "strategy_id".into(),
                json!({ "from": current.strategy_id, "to": value }),
            );
            next.strategy_id = value;
        }
        if let Some(value) = command.symbol {
            let value = value.trim().to_string();
            if value.is_empty() {
                return Err(TradeUseCaseError::Validation(
                    "symbol must not be empty".into(),
                ));
            }
            diff.insert(
                "symbol".into(),
                json!({ "from": current.symbol, "to": value }),
            );
            next.symbol = value;
        }
        if let Some(value) = command.side {
            validate_side(&value)?;
            diff.insert("side".into(), json!({ "from": current.side, "to": value }));
            next.side = value;
        }
        if let Some(value) = command.qty {
            if value <= Decimal::ZERO {
                return Err(TradeUseCaseError::Validation("qty must be positive".into()));
            }
            diff.insert("qty".into(), json!({ "from": current.qty, "to": value }));
            next.qty = value;
        }
        if let Some(value) = command.price {
            diff.insert(
                "price".into(),
                json!({ "from": current.price, "to": value }),
            );
            next.price = value;
        }
        if let Some(value) = command.fee {
            diff.insert("fee".into(), json!({ "from": current.fee, "to": value }));
            next.fee = value;
        }
        if let Some(value) = command.date {
            diff.insert("date".into(), json!({ "from": current.date, "to": value }));
            next.date = value;
        }
        if let Some(value) = command.source {
            validate_source(&value)?;
            diff.insert(
                "source".into(),
                json!({ "from": current.source, "to": value }),
            );
            next.source = value;
        }
        if let Some(value) = command.note {
            diff.insert("note".into(), json!({ "from": current.note, "to": value }));
            next.note = Some(value);
        }
        next.updated_at = Utc::now().fixed_offset();

        let updated = self
            .repository
            .update(&transaction, TradeUpdate { trade: next })
            .await?;
        if !diff.is_empty() {
            self.change_history
                .record(
                    &transaction,
                    ChangeHistoryRecord {
                        actor: Actor::Human,
                        target_kind: TargetKind::Trade,
                        target_id: id,
                        op: Op::Update,
                        diff: Value::Object(diff),
                        summary: None,
                    },
                )
                .await?;
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(updated)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), TradeUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        if !self.repository.delete(&transaction, id).await? {
            return Err(TradeUseCaseError::NotFound(id));
        }
        self.change_history
            .record(
                &transaction,
                ChangeHistoryRecord {
                    actor: Actor::Human,
                    target_kind: TargetKind::Trade,
                    target_id: id,
                    op: Op::Delete,
                    diff: json!({}),
                    summary: None,
                },
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    pub async fn summary(
        &self,
        strategy_id: Option<Uuid>,
    ) -> Result<PerformanceSummary, TradeUseCaseError> {
        let rows = self
            .repository
            .list(TradeQuery {
                strategy_id,
                symbol: None,
                date_from: None,
                limit: None,
                order: TradeOrder::DateAscending,
                include_note_count: false,
                include_note_references: false,
            })
            .await?;
        Ok(summarize(strategy_id, &rows))
    }

    async fn ensure_strategy_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<(), TradeUseCaseError> {
        if !self
            .strategy_existence
            .exists(transaction, strategy_id)
            .await?
        {
            return Err(TradeUseCaseError::Validation(format!(
                "strategy {strategy_id} does not exist"
            )));
        }
        Ok(())
    }
}

fn validate_side(side: &str) -> Result<(), TradeUseCaseError> {
    if ALLOWED_SIDE.contains(&side) {
        Ok(())
    } else {
        Err(TradeUseCaseError::Validation(format!(
            "invalid side: {side}"
        )))
    }
}

fn validate_source(source: &str) -> Result<(), TradeUseCaseError> {
    if ALLOWED_SOURCE.contains(&source) {
        Ok(())
    } else {
        Err(TradeUseCaseError::Validation(format!(
            "invalid source: {source}"
        )))
    }
}

fn summarize(strategy_id: Option<Uuid>, rows: &[TradeListItem]) -> PerformanceSummary {
    #[derive(Debug, Clone)]
    struct Lot {
        qty: Decimal,
        price: Decimal,
    }

    let mut lots_by_symbol: HashMap<String, VecDeque<Lot>> = HashMap::new();
    let mut realized_by_symbol: HashMap<String, Decimal> = HashMap::new();

    for row in rows {
        let trade = &row.trade;
        let lots = lots_by_symbol.entry(trade.symbol.clone()).or_default();
        let realized = realized_by_symbol.entry(trade.symbol.clone()).or_default();
        match trade.side.as_str() {
            "buy" => {
                lots.push_back(Lot {
                    qty: trade.qty,
                    price: trade.price,
                });
                *realized -= trade.fee;
            }
            "sell" => {
                let mut remaining = trade.qty;
                while remaining > Decimal::ZERO {
                    let Some(front) = lots.front_mut() else {
                        *realized += remaining * trade.price;
                        break;
                    };
                    let taken = remaining.min(front.qty);
                    *realized += taken * (trade.price - front.price);
                    front.qty -= taken;
                    remaining -= taken;
                    if front.qty == Decimal::ZERO {
                        lots.pop_front();
                    }
                }
                *realized -= trade.fee;
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
    use std::sync::Arc;

    use chrono::NaiveDate;
    use rstest::rstest;
    use serde_json::json;

    use crate::change_history::{Actor, FakeChangeHistory, Op, TargetKind};
    use crate::strategy_existence::FakeStrategyExistence;
    use crate::trade::FakeTradeRepository;
    use crate::trade::types::{PerformanceSummary, PositionSummary, Trade, TradeListItem};
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{CreateTradeCommand, TradeUseCases, summarize};

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

    #[tokio::test]
    async fn create_uses_one_transaction_across_repository_strategy_existence_and_history() {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeTradeRepository::new());
        let strategy_existence = Arc::new(FakeStrategyExistence::new());
        let change_history = Arc::new(FakeChangeHistory::new());
        let strategy_id = uuid::Uuid::nil();
        strategy_existence.insert_strategy(strategy_id).await;
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let trade_use_cases = TradeUseCases::new(
            shared_unit_of_work,
            repository.clone(),
            strategy_existence.clone(),
            change_history.clone(),
        );

        let trade = trade_use_cases
            .create(CreateTradeCommand {
                strategy_id,
                symbol: "FICTIONAL-SYMBOL".into(),
                side: "buy".into(),
                qty: rust_decimal::Decimal::from(3),
                price: rust_decimal::Decimal::from(25),
                fee: None,
                date: NaiveDate::MIN,
                source: "manual".into(),
                note: None,
            })
            .await
            .expect("trade creation succeeds");
        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();
        let repository_transactions = repository.transaction_ids.lock().await.clone();
        let strategy_transactions = strategy_existence.transaction_ids().await;
        let history = change_history.entries.lock().await.clone();
        let transaction_id = begun.first().copied().unwrap_or_default();

        assert_eq!(
            (
                begun.len(),
                committed,
                repository_transactions
                    .iter()
                    .map(|id| *id == transaction_id)
                    .collect::<Vec<_>>(),
                strategy_transactions
                    .iter()
                    .map(|id| *id == transaction_id)
                    .collect::<Vec<_>>(),
                history
                    .into_iter()
                    .map(|entry| (
                        entry.transaction_id == transaction_id,
                        entry.record.actor,
                        entry.record.target_kind,
                        entry.record.target_id,
                        entry.record.op,
                        entry.record.diff,
                        entry.record.summary,
                    ))
                    .collect::<Vec<_>>(),
            ),
            (
                1,
                vec![transaction_id],
                vec![true],
                vec![true],
                vec![(
                    true,
                    Actor::Human,
                    TargetKind::Trade,
                    trade.id,
                    Op::Create,
                    json!({
                        "strategy_id": strategy_id,
                        "symbol": "FICTIONAL-SYMBOL",
                        "side": "buy",
                        "qty": 3,
                        "price": 25,
                    }),
                    None,
                )],
            ),
        );
    }
}
