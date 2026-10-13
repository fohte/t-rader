use std::collections::BTreeMap;

use crate::bars::SharedBarsRepository;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, TargetKind};
use crate::split_adjustment::{SplitAdjustment, SplitCursor};
use crate::strategy_existence::SharedStrategyExistence;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkTransaction};

use super::error::TradeUseCaseError;
use super::fifo::{calculate_trade_detail, split_adjustment_error, summarize};
use super::repository::SharedTradeRepository;
use super::types::{
    CreateTradeCommand, NewTrade, PerformanceSummary, Trade, TradeDetail, TradeListItem,
    TradeOrder, TradeQuery, TradeUpdate, TradeUpdateCommand,
};

const ALLOWED_SIDE: [&str; 2] = ["buy", "sell"];
const ALLOWED_SOURCE: [&str; 3] = ["manual", "csv", "api"];

#[derive(Clone)]
pub struct TradeUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedTradeRepository,
    pub(super) bars_repository: SharedBarsRepository,
    pub(super) strategy_existence: SharedStrategyExistence,
    pub(super) change_history: crate::change_history::SharedChangeHistoryPort,
}

impl TradeUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedTradeRepository,
        strategy_existence: SharedStrategyExistence,
        change_history: crate::change_history::SharedChangeHistoryPort,
        bars_repository: SharedBarsRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
            bars_repository,
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

    pub async fn get_detail(&self, id: Uuid) -> Result<TradeDetail, TradeUseCaseError> {
        let trade = self.get(id).await?;
        let rows = self
            .repository
            .list(TradeQuery {
                strategy_id: None,
                symbol: Some(trade.symbol.clone()),
                date_from: None,
                limit: None,
                order: TradeOrder::DateAscending,
                include_note_count: false,
                include_note_references: false,
            })
            .await?;
        let split_cursor = self.split_cursor_for_rows(&rows).await?;
        calculate_trade_detail(trade, &rows, split_cursor)
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
        let split_cursor = self.split_cursor_for_rows(&rows).await?;
        summarize(strategy_id, &rows, split_cursor)
    }

    async fn split_cursor_for_rows(
        &self,
        rows: &[TradeListItem],
    ) -> Result<SplitCursor, TradeUseCaseError> {
        let mut adjustments = Vec::new();
        let mut from_by_symbol = BTreeMap::<String, NaiveDate>::new();
        for row in rows {
            from_by_symbol
                .entry(row.trade.symbol.clone())
                .and_modify(|from| *from = (*from).min(row.trade.date))
                .or_insert(row.trade.date);
        }
        for (symbol, from) in from_by_symbol {
            adjustments.extend(
                self.bars_repository
                    .find_daily_adjustment_factors_from(&symbol, from)
                    .await?
                    .into_iter()
                    .map(|adjustment| SplitAdjustment {
                        instrument_id: symbol.clone(),
                        date: adjustment.date,
                        factor: adjustment.factor,
                    }),
            );
        }
        SplitCursor::new(adjustments.clone()).map_err(|error| {
            match adjustments
                .iter()
                .find(|adjustment| adjustment.factor <= Decimal::ZERO)
            {
                Some(adjustment) => split_adjustment_error(error, adjustment),
                None => TradeUseCaseError::FifoCalculation(error.to_string()),
            }
        })
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

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use crate::bars::{DailyBarAdjustmentFactor, FakeBarsRepository};
    use chrono::NaiveDate;
    use serde_json::json;

    use crate::change_history::{Actor, FakeChangeHistory, Op, TargetKind};
    use crate::strategy_existence::FakeStrategyExistence;
    use crate::trade::FakeTradeRepository;
    use crate::trade::types::{
        MatchedBuy, PerformanceSummary, PositionSummary, Trade, TradeDetail,
    };
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{CreateTradeCommand, TradeUseCases};

    fn dated_trade(id: u128, date: NaiveDate, side: &str, qty: i64, price: i64) -> Trade {
        dated_trade_for_symbol(id, date, "FICTIONAL-SYMBOL", side, qty, price)
    }

    fn dated_trade_for_symbol(
        id: u128,
        date: NaiveDate,
        symbol: &str,
        side: &str,
        qty: i64,
        price: i64,
    ) -> Trade {
        let timestamp = date
            .and_time(chrono::NaiveTime::MIN)
            .and_utc()
            .fixed_offset();
        Trade {
            id: uuid::Uuid::from_u128(id),
            strategy_id: uuid::Uuid::nil(),
            symbol: symbol.into(),
            side: side.into(),
            qty: rust_decimal::Decimal::from(qty),
            price: rust_decimal::Decimal::from(price),
            fee: rust_decimal::Decimal::ZERO,
            date,
            source: "manual".into(),
            note: None,
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    fn trade_use_cases(
        repository: Arc<FakeTradeRepository>,
        bars_repository: Arc<FakeBarsRepository>,
    ) -> TradeUseCases {
        TradeUseCases::new(
            Arc::new(FakeUnitOfWork::new()),
            repository,
            Arc::new(FakeStrategyExistence::new()),
            Arc::new(FakeChangeHistory::new()),
            bars_repository,
        )
    }

    #[tokio::test]
    async fn summary_adjusts_fifo_lots_for_splits_before_and_after_trades() {
        let before_first_split = NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid date");
        let first_split_date = NaiveDate::from_ymd_opt(2026, 1, 6).expect("valid date");
        let after_first_split = NaiveDate::from_ymd_opt(2026, 1, 7).expect("valid date");
        let final_trade_date = NaiveDate::from_ymd_opt(2026, 1, 8).expect("valid date");
        let final_split_date = NaiveDate::from_ymd_opt(2026, 1, 9).expect("valid date");
        let repository = Arc::new(FakeTradeRepository::new());
        for trade in [
            dated_trade(1, before_first_split, "buy", 100, 1000),
            dated_trade(2, first_split_date, "sell", 100, 600),
            dated_trade(3, after_first_split, "buy", 100, 700),
            dated_trade(4, final_trade_date, "sell", 150, 800),
        ] {
            repository.insert_trade(trade).await;
        }
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_daily_adjustment_factors(
                "FICTIONAL-SYMBOL",
                vec![
                    DailyBarAdjustmentFactor {
                        date: first_split_date,
                        factor: rust_decimal::Decimal::new(5, 1),
                    },
                    DailyBarAdjustmentFactor {
                        date: final_split_date,
                        factor: rust_decimal::Decimal::new(25, 2),
                    },
                ],
            )
            .await;
        let trade_use_cases = trade_use_cases(repository, bars_repository);

        let summary = trade_use_cases
            .summary(None)
            .await
            .expect("summary succeeds");

        assert_eq!(
            summary,
            PerformanceSummary {
                strategy_id: None,
                trade_count: 4,
                realized_pnl: rust_decimal::Decimal::from(45_000),
                positions: vec![PositionSummary {
                    symbol: "FICTIONAL-SYMBOL".into(),
                    qty: rust_decimal::Decimal::from(200),
                    avg_cost: rust_decimal::Decimal::from(175),
                    cost_basis: rust_decimal::Decimal::from(35_000),
                    realized_pnl: rust_decimal::Decimal::from(45_000),
                }],
            },
        );
    }

    #[tokio::test]
    async fn summary_applies_each_instruments_own_split_history() {
        let first_trade_date = NaiveDate::from_ymd_opt(2026, 2, 2).expect("valid date");
        let first_split_date = NaiveDate::from_ymd_opt(2026, 2, 3).expect("valid date");
        let first_sale_date = NaiveDate::from_ymd_opt(2026, 2, 4).expect("valid date");
        let second_split_date = NaiveDate::from_ymd_opt(2026, 2, 5).expect("valid date");
        let second_sale_date = NaiveDate::from_ymd_opt(2026, 2, 6).expect("valid date");
        let repository = Arc::new(FakeTradeRepository::new());
        for trade in [
            dated_trade_for_symbol(1, first_trade_date, "FICTIONAL-A", "buy", 100, 1000),
            dated_trade_for_symbol(2, first_trade_date, "FICTIONAL-B", "buy", 40, 600),
            dated_trade_for_symbol(3, first_sale_date, "FICTIONAL-A", "sell", 50, 600),
            dated_trade_for_symbol(4, second_sale_date, "FICTIONAL-B", "sell", 80, 200),
        ] {
            repository.insert_trade(trade).await;
        }
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_daily_adjustment_factors(
                "FICTIONAL-A",
                vec![DailyBarAdjustmentFactor {
                    date: first_split_date,
                    factor: rust_decimal::Decimal::new(5, 1),
                }],
            )
            .await;
        bars_repository
            .seed_daily_adjustment_factors(
                "FICTIONAL-B",
                vec![DailyBarAdjustmentFactor {
                    date: second_split_date,
                    factor: rust_decimal::Decimal::new(25, 2),
                }],
            )
            .await;

        let summary = trade_use_cases(repository, bars_repository)
            .summary(None)
            .await
            .expect("summary succeeds");

        assert_eq!(
            summary,
            PerformanceSummary {
                strategy_id: None,
                trade_count: 4,
                realized_pnl: rust_decimal::Decimal::from(9000),
                positions: vec![
                    PositionSummary {
                        symbol: "FICTIONAL-A".into(),
                        qty: rust_decimal::Decimal::from(150),
                        avg_cost: rust_decimal::Decimal::from(500),
                        cost_basis: rust_decimal::Decimal::from(75_000),
                        realized_pnl: rust_decimal::Decimal::from(5000),
                    },
                    PositionSummary {
                        symbol: "FICTIONAL-B".into(),
                        qty: rust_decimal::Decimal::from(80),
                        avg_cost: rust_decimal::Decimal::from(150),
                        cost_basis: rust_decimal::Decimal::from(12_000),
                        realized_pnl: rust_decimal::Decimal::from(4000),
                    },
                ],
            },
        );
    }

    #[tokio::test]
    async fn summary_closes_a_position_after_a_split_with_a_rounded_factor() {
        let buy_date = NaiveDate::from_ymd_opt(2026, 3, 2).expect("valid date");
        let split_date = NaiveDate::from_ymd_opt(2026, 3, 3).expect("valid date");
        let sell_date = NaiveDate::from_ymd_opt(2026, 3, 4).expect("valid date");
        let repository = Arc::new(FakeTradeRepository::new());
        for trade in [
            dated_trade(1, buy_date, "buy", 100, 90),
            dated_trade(2, sell_date, "sell", 300, 30),
        ] {
            repository.insert_trade(trade).await;
        }
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_daily_adjustment_factors(
                "FICTIONAL-SYMBOL",
                vec![DailyBarAdjustmentFactor {
                    date: split_date,
                    factor: rust_decimal::Decimal::new(333_333, 6),
                }],
            )
            .await;

        let summary = trade_use_cases(repository, bars_repository)
            .summary(None)
            .await
            .expect("summary succeeds");

        assert_eq!(
            summary,
            PerformanceSummary {
                strategy_id: None,
                trade_count: 2,
                realized_pnl: rust_decimal::Decimal::new(9, 3),
                positions: vec![],
            },
        );
    }

    #[tokio::test]
    async fn detail_keeps_buy_execution_units_across_splits_before_and_after_sale() {
        let buy_date = NaiveDate::from_ymd_opt(2026, 4, 1).expect("valid date");
        let split_date = NaiveDate::from_ymd_opt(2026, 4, 2).expect("valid date");
        let sell_date = NaiveDate::from_ymd_opt(2026, 4, 3).expect("valid date");
        let current_split_date = NaiveDate::from_ymd_opt(2026, 4, 4).expect("valid date");
        let buy = dated_trade(1, buy_date, "buy", 100, 1000);
        let sell = dated_trade(2, sell_date, "sell", 150, 600);
        let repository = Arc::new(FakeTradeRepository::new());
        repository.insert_trade(buy.clone()).await;
        repository.insert_trade(sell.clone()).await;
        let bars_repository = Arc::new(FakeBarsRepository::new());
        bars_repository
            .seed_daily_adjustment_factors(
                "FICTIONAL-SYMBOL",
                vec![
                    DailyBarAdjustmentFactor {
                        date: split_date,
                        factor: rust_decimal::Decimal::new(5, 1),
                    },
                    DailyBarAdjustmentFactor {
                        date: current_split_date,
                        factor: rust_decimal::Decimal::new(25, 2),
                    },
                ],
            )
            .await;
        let trade_use_cases = trade_use_cases(repository, bars_repository);

        let summary = trade_use_cases
            .summary(None)
            .await
            .expect("summary succeeds");
        let buy_detail = trade_use_cases
            .get_detail(buy.id)
            .await
            .expect("buy detail succeeds");
        let sell_detail = trade_use_cases
            .get_detail(sell.id)
            .await
            .expect("sell detail succeeds");

        assert_eq!(
            (summary, buy_detail, sell_detail),
            (
                PerformanceSummary {
                    strategy_id: None,
                    trade_count: 2,
                    realized_pnl: rust_decimal::Decimal::from(15_000),
                    positions: vec![PositionSummary {
                        symbol: "FICTIONAL-SYMBOL".into(),
                        qty: rust_decimal::Decimal::from(200),
                        avg_cost: rust_decimal::Decimal::from(125),
                        cost_basis: rust_decimal::Decimal::from(25_000),
                        realized_pnl: rust_decimal::Decimal::from(15_000),
                    }],
                },
                TradeDetail {
                    trade: buy,
                    realized_pnl: None,
                    matched_buys: None,
                    remaining_qty: Some(rust_decimal::Decimal::from(25)),
                },
                TradeDetail {
                    trade: sell,
                    realized_pnl: Some(rust_decimal::Decimal::from(15_000)),
                    matched_buys: Some(vec![MatchedBuy {
                        date: buy_date,
                        qty: rust_decimal::Decimal::from(75),
                        price: rust_decimal::Decimal::from(1000),
                    }]),
                    remaining_qty: None,
                },
            ),
        );
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
            Arc::new(FakeBarsRepository::new()),
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
