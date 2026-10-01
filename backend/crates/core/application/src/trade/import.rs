use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;
use serde_json::json;

use crate::change_history::{Actor, ChangeHistoryRecord, Op, TargetKind};

use super::error::TradeUseCaseError;
use super::types::{NewTrade, SbiImportResult, SbiImportRow, TradeMatchQuery};
use super::use_cases::TradeUseCases;

const ALLOWED_SIDE: [&str; 2] = ["buy", "sell"];

impl TradeUseCases {
    pub async fn count_import_matches(
        &self,
        query: &TradeMatchQuery,
    ) -> Result<usize, TradeUseCaseError> {
        self.repository
            .count_matching_trades(query)
            .await
            .map_err(Into::into)
    }

    pub async fn commit_sbi_import(
        &self,
        rows: Vec<SbiImportRow>,
    ) -> Result<SbiImportResult, TradeUseCaseError> {
        for row in &rows {
            validate_import_row(row)?;
        }

        let transaction = self.unit_of_work.begin().await?;
        let mut checked_strategies = HashSet::new();
        for row in &rows {
            if checked_strategies.insert(row.strategy_id)
                && !self
                    .strategy_existence
                    .exists(&transaction, row.strategy_id)
                    .await?
            {
                return Err(TradeUseCaseError::Validation(format!(
                    "strategy {} does not exist",
                    row.strategy_id
                )));
            }
        }

        let mut imported_count = 0;
        let mut skipped_count = 0;
        let mut csv_counts: HashMap<TradeMatchQuery, usize> = HashMap::new();
        let mut database_counts: HashMap<TradeMatchQuery, usize> = HashMap::new();

        for row in rows {
            let key = TradeMatchQuery {
                date: row.date,
                symbol: row.symbol.clone(),
                side: row.side.clone(),
                qty: row.qty,
                price: row.price,
            };
            let csv_index = {
                let count = csv_counts.entry(key.clone()).or_default();
                *count += 1;
                *count
            };
            let database_count = match database_counts.get(&key) {
                Some(count) => *count,
                None => {
                    let count = self
                        .repository
                        .count_matching_trades_in_transaction(&transaction, &key)
                        .await?;
                    database_counts.insert(key, count);
                    count
                }
            };
            if csv_index <= database_count {
                skipped_count += 1;
                continue;
            }

            self.repository
                .ensure_stock(&transaction, &row.symbol, &row.stock_name)
                .await?;
            let id = uuid::Uuid::new_v4();
            self.repository
                .insert(
                    &transaction,
                    NewTrade {
                        id,
                        strategy_id: row.strategy_id,
                        symbol: row.symbol.clone(),
                        side: row.side.clone(),
                        qty: row.qty,
                        price: row.price,
                        fee: row.fee.unwrap_or(Decimal::ZERO),
                        date: row.date,
                        source: "csv".into(),
                        note: None,
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
                            "strategy_id": row.strategy_id,
                            "symbol": row.symbol,
                            "side": row.side,
                            "qty": row.qty,
                            "price": row.price,
                            "source": "csv",
                            "origin": "sbi_csv_import",
                        }),
                        summary: None,
                    },
                )
                .await?;
            imported_count += 1;
        }

        self.unit_of_work.commit(transaction).await?;
        Ok(SbiImportResult {
            imported_count,
            skipped_count,
        })
    }
}

fn validate_import_row(row: &SbiImportRow) -> Result<(), TradeUseCaseError> {
    if row.symbol.trim().is_empty() {
        return Err(TradeUseCaseError::Validation(
            "symbol must not be empty".into(),
        ));
    }
    if !ALLOWED_SIDE.contains(&row.side.as_str()) {
        return Err(TradeUseCaseError::Validation(format!(
            "invalid side: {}",
            row.side
        )));
    }
    if row.qty <= Decimal::ZERO {
        return Err(TradeUseCaseError::Validation("qty must be positive".into()));
    }
    if row.price < Decimal::ZERO {
        return Err(TradeUseCaseError::Validation(
            "price must be non-negative".into(),
        ));
    }
    if row.fee.is_some_and(|fee| fee < Decimal::ZERO) {
        return Err(TradeUseCaseError::Validation(
            "fee must be non-negative".into(),
        ));
    }
    Ok(())
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use chrono::NaiveDate;
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use serde_json::json;
    use uuid::Uuid;

    use crate::change_history::{Actor, ChangeHistoryRecord, FakeChangeHistory, Op, TargetKind};
    use crate::strategy_existence::FakeStrategyExistence;
    use crate::trade::{FakeTradeRepository, SbiImportResult, SbiImportRow, Trade, TradeUseCases};
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    struct ImportFixture {
        use_cases: TradeUseCases,
        repository: Arc<FakeTradeRepository>,
        strategy_existence: Arc<FakeStrategyExistence>,
        unit_of_work: Arc<FakeUnitOfWork>,
        change_history: Arc<FakeChangeHistory>,
        strategy_id: Uuid,
        date: NaiveDate,
    }

    impl ImportFixture {
        async fn seed(&self) {
            self.strategy_existence
                .insert_strategy(self.strategy_id)
                .await;
            self.repository
                .insert_trade(Trade {
                    id: Uuid::from_u128(1),
                    strategy_id: self.strategy_id,
                    symbol: "ZXQ91".into(),
                    side: "buy".into(),
                    qty: Decimal::from(3),
                    price: Decimal::from(25),
                    fee: Decimal::ZERO,
                    date: self.date,
                    source: "manual".into(),
                    note: None,
                    created_at: chrono::Utc::now().fixed_offset(),
                    updated_at: chrono::Utc::now().fixed_offset(),
                })
                .await;
        }

        fn duplicate_rows(&self) -> Vec<SbiImportRow> {
            (0..3)
                .map(|_| SbiImportRow {
                    strategy_id: self.strategy_id,
                    date: self.date,
                    symbol: "ZXQ91".into(),
                    stock_name: "Example Placeholder".into(),
                    side: "buy".into(),
                    qty: Decimal::from(3),
                    price: Decimal::from(25),
                    fee: None,
                })
                .collect()
        }

        fn unique_row(&self) -> SbiImportRow {
            SbiImportRow {
                strategy_id: self.strategy_id,
                date: self.date,
                symbol: "QWE42".into(),
                stock_name: "Another Placeholder".into(),
                side: "buy".into(),
                qty: Decimal::from(2),
                price: Decimal::from(18),
                fee: None,
            }
        }
    }

    #[fixture]
    fn import_fixture() -> ImportFixture {
        let repository = Arc::new(FakeTradeRepository::new());
        let strategy_existence = Arc::new(FakeStrategyExistence::new());
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let change_history = Arc::new(FakeChangeHistory::new());
        let strategy_id = Uuid::nil();
        let use_cases = TradeUseCases::new(
            unit_of_work.clone() as SharedUnitOfWork,
            repository.clone(),
            strategy_existence.clone(),
            change_history.clone(),
        );
        ImportFixture {
            use_cases,
            repository,
            strategy_existence,
            unit_of_work,
            change_history,
            strategy_id,
            date: NaiveDate::MIN,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn commit_import_preserves_duplicate_counts_and_stock_values(
        import_fixture: ImportFixture,
    ) {
        import_fixture.seed().await;

        let result = import_fixture
            .use_cases
            .commit_sbi_import(import_fixture.duplicate_rows())
            .await
            .expect("import succeeds");

        let imported_trades: Vec<_> = import_fixture
            .repository
            .trades
            .lock()
            .await
            .values()
            .filter(|trade| trade.id != Uuid::from_u128(1))
            .cloned()
            .collect();
        let mut imported_rows: Vec<_> = imported_trades
            .iter()
            .map(|trade| {
                (
                    trade.strategy_id,
                    trade.symbol.clone(),
                    trade.side.clone(),
                    trade.qty,
                    trade.price,
                    trade.fee,
                    trade.date,
                    trade.source.clone(),
                    trade.note.clone(),
                )
            })
            .collect();
        imported_rows.sort_by(|left, right| left.1.cmp(&right.1));
        let stocks = import_fixture.repository.stocks.lock().await.clone();

        assert_eq!(
            (result, imported_rows, stocks),
            (
                SbiImportResult {
                    imported_count: 2,
                    skipped_count: 1,
                },
                vec![
                    (
                        import_fixture.strategy_id,
                        "ZXQ91".into(),
                        "buy".into(),
                        Decimal::from(3),
                        Decimal::from(25),
                        Decimal::ZERO,
                        import_fixture.date,
                        "csv".into(),
                        None,
                    );
                    2
                ],
                HashMap::from([("ZXQ91".into(), "Example Placeholder".into())]),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn commit_import_uses_one_transaction_for_trade_and_history(
        import_fixture: ImportFixture,
    ) {
        import_fixture.seed().await;
        let row = import_fixture.unique_row();
        import_fixture
            .use_cases
            .commit_sbi_import(vec![row.clone()])
            .await
            .expect("import succeeds");

        let begun = import_fixture.unit_of_work.begun.lock().await.clone();
        let committed = import_fixture.unit_of_work.committed.lock().await.clone();
        let repository_transactions = import_fixture
            .repository
            .transaction_ids
            .lock()
            .await
            .clone();
        let strategy_transactions = import_fixture.strategy_existence.transaction_ids().await;
        let history = import_fixture.change_history.entries.lock().await.clone();
        let transaction_id = begun.first().copied().unwrap_or_default();
        let imported_trade_id = import_fixture
            .repository
            .trades
            .lock()
            .await
            .values()
            .find(|trade| trade.symbol == row.symbol)
            .expect("imported trade exists")
            .id;
        let history_records = history
            .iter()
            .map(|entry| {
                let target_matches_import = entry.record.target_id == imported_trade_id;
                let mut record = entry.record.clone();
                record.target_id = Uuid::nil();
                (
                    record,
                    entry.transaction_id == transaction_id,
                    target_matches_import,
                )
            })
            .collect::<Vec<_>>();

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
                history_records,
            ),
            (
                1,
                vec![transaction_id],
                vec![true; 3],
                vec![true],
                vec![(
                    ChangeHistoryRecord {
                        actor: Actor::Human,
                        target_kind: TargetKind::Trade,
                        target_id: Uuid::nil(),
                        op: Op::Create,
                        diff: json!({
                            "strategy_id": import_fixture.strategy_id,
                            "symbol": row.symbol,
                            "side": row.side,
                            "qty": row.qty,
                            "price": row.price,
                            "source": "csv",
                            "origin": "sbi_csv_import",
                        }),
                        summary: None,
                    },
                    true,
                    true,
                ),],
            ),
        );
    }
}
