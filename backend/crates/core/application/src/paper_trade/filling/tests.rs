#![allow(clippy::expect_used, reason = "テスト実行時に失敗理由を表示するため")]

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    bars::FakeBarsRepository,
    paper_trade::{
        NewPaperAccount, NewPaperOrder, PaperAccount, PaperOrder, PaperOrderRejectReason,
        PaperOrderResult, PaperOrderSide, PaperOrderWithResult, PaperTradeFillStats,
        PaperTradeRepository, PaperTradeRepositoryError, PaperTradeUseCases,
    },
    unit_of_work::{FakeTransaction, FakeUnitOfWork},
};

const ACCOUNT_ID: Uuid = Uuid::from_u128(1);
const FIRST_ORDER_ID: Uuid = Uuid::from_u128(2);
const SECOND_ORDER_ID: Uuid = Uuid::from_u128(3);
const THIRD_ORDER_ID: Uuid = Uuid::from_u128(4);
const NOTE_VERSION_ID: Uuid = Uuid::from_u128(5);
const STOCK_ID: &str = "DEMO-A1";

#[derive(Default)]
struct FakePaperTradeRepository {
    accounts: Mutex<Vec<PaperAccount>>,
    orders: Mutex<Vec<PaperOrderWithResult>>,
    inserted_orders: Mutex<Vec<NewPaperOrder>>,
    result_transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakePaperTradeRepository {
    fn empty() -> Self {
        Self::with_accounts(Vec::new(), Vec::new())
    }

    fn new(account: PaperAccount, orders: Vec<PaperOrder>) -> Self {
        let orders = orders
            .into_iter()
            .map(|order| PaperOrderWithResult {
                order,
                result: None,
            })
            .collect();
        Self::with_accounts(vec![account], orders)
    }

    fn with_results(account: PaperAccount, orders: Vec<PaperOrderWithResult>) -> Self {
        Self::with_accounts(vec![account], orders)
    }

    fn with_accounts(accounts: Vec<PaperAccount>, orders: Vec<PaperOrderWithResult>) -> Self {
        Self {
            accounts: Mutex::new(accounts),
            orders: Mutex::new(orders),
            inserted_orders: Mutex::new(Vec::new()),
            result_transaction_ids: Mutex::new(Vec::new()),
        }
    }

    async fn accounts(&self) -> Vec<PaperAccount> {
        self.accounts.lock().await.clone()
    }

    async fn results(&self) -> Vec<Option<PaperOrderResult>> {
        self.orders
            .lock()
            .await
            .iter()
            .map(|item| item.result.clone())
            .collect()
    }

    async fn inserted_orders(&self) -> Vec<NewPaperOrder> {
        self.inserted_orders.lock().await.clone()
    }

    async fn result_transaction_ids(&self) -> Vec<Uuid> {
        self.result_transaction_ids.lock().await.clone()
    }
}

#[async_trait]
impl PaperTradeRepository for FakePaperTradeRepository {
    async fn account_for_strategy_purpose(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        strategy_id: Uuid,
        purpose: &str,
    ) -> Result<Option<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self
            .accounts
            .lock()
            .await
            .iter()
            .find(|account| account.strategy_id == strategy_id && account.purpose == purpose)
            .cloned())
    }

    async fn find_account(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        account_id: Uuid,
    ) -> Result<Option<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self
            .accounts
            .lock()
            .await
            .iter()
            .find(|account| account.id == account_id)
            .cloned())
    }

    async fn list_accounts(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self.accounts.lock().await.clone())
    }

    async fn list_orders_with_results(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        account_id: Option<Uuid>,
    ) -> Result<Vec<PaperOrderWithResult>, PaperTradeRepositoryError> {
        Ok(self
            .orders
            .lock()
            .await
            .iter()
            .filter(|item| account_id.is_none_or(|id| item.order.account_id == id))
            .cloned()
            .collect())
    }

    async fn insert_account(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        account: NewPaperAccount,
    ) -> Result<PaperAccount, PaperTradeRepositoryError> {
        let account = PaperAccount {
            id: ACCOUNT_ID,
            name: account.name,
            strategy_id: account.strategy_id,
            purpose: account.purpose,
            initial_cash_jpy: account.initial_cash_jpy,
            benchmark_stock_id: account.benchmark_stock_id,
            started_on: account.started_on,
        };
        self.accounts.lock().await.push(account.clone());
        Ok(account)
    }

    async fn insert_order(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        order: NewPaperOrder,
    ) -> Result<PaperOrder, PaperTradeRepositoryError> {
        self.inserted_orders.lock().await.push(order.clone());
        let order = PaperOrder {
            id: FIRST_ORDER_ID,
            account_id: order.account_id,
            stock_id: order.stock_id,
            side: order.side,
            qty: order.qty,
            note_version_id: order.note_version_id,
            ordered_at: decided_at(5),
        };
        self.orders.lock().await.push(PaperOrderWithResult {
            order: order.clone(),
            result: None,
        });
        Ok(order)
    }

    async fn lock_accounts_for_filling(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self.accounts.lock().await.clone())
    }

    async fn list_orders_for_filling(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
    ) -> Result<Vec<PaperOrderWithResult>, PaperTradeRepositoryError> {
        Ok(self.orders.lock().await.clone())
    }

    async fn insert_result(
        &self,
        transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        result: PaperOrderResult,
    ) -> Result<(), PaperTradeRepositoryError> {
        let Some(transaction_id) = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
        else {
            return Err(PaperTradeRepositoryError::InvalidTransaction);
        };
        self.result_transaction_ids
            .lock()
            .await
            .push(transaction_id);
        let mut orders = self.orders.lock().await;
        let Some(item) = orders
            .iter_mut()
            .find(|item| item.order.id == result.order_id())
        else {
            return Err(PaperTradeRepositoryError::InvalidTransaction);
        };
        item.result = Some(result);
        Ok(())
    }

    async fn account_exists(
        &self,
        _transaction: &crate::unit_of_work::UnitOfWorkTransaction,
        account_id: Uuid,
    ) -> Result<bool, PaperTradeRepositoryError> {
        Ok(self
            .accounts
            .lock()
            .await
            .iter()
            .any(|account| account.id == account_id))
    }
}

fn use_cases(
    repository: Arc<FakePaperTradeRepository>,
    bars: Arc<FakeBarsRepository>,
) -> PaperTradeUseCases {
    use_cases_with_unit_of_work(repository, bars).0
}

fn use_cases_with_unit_of_work(
    repository: Arc<FakePaperTradeRepository>,
    bars: Arc<FakeBarsRepository>,
) -> (PaperTradeUseCases, Arc<FakeUnitOfWork>) {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    (
        PaperTradeUseCases::new(unit_of_work.clone(), repository, bars),
        unit_of_work,
    )
}

async fn fill_and_snapshot(
    use_cases: &PaperTradeUseCases,
    repository: &FakePaperTradeRepository,
    decided_at: DateTime<FixedOffset>,
) -> (PaperTradeFillStats, Vec<Option<PaperOrderResult>>) {
    let stats = use_cases
        .fill_pending_orders(decided_at)
        .await
        .expect("fill operation succeeds");
    (stats, repository.results().await)
}

fn account(initial_cash: i64) -> PaperAccount {
    PaperAccount {
        id: ACCOUNT_ID,
        name: "Demo account".to_string(),
        strategy_id: Uuid::from_u128(6),
        purpose: "demo-purpose".to_string(),
        initial_cash_jpy: Decimal::from(initial_cash),
        benchmark_stock_id: None,
        started_on: date(1),
    }
}

fn new_account(
    name: &str,
    purpose: &str,
    initial_cash_jpy: i64,
    benchmark_stock_id: Option<&str>,
) -> NewPaperAccount {
    NewPaperAccount {
        name: name.to_string(),
        strategy_id: Uuid::from_u128(6),
        purpose: purpose.to_string(),
        initial_cash_jpy: Decimal::from(initial_cash_jpy),
        benchmark_stock_id: benchmark_stock_id.map(str::to_string),
        started_on: date(1),
    }
}

fn new_order(stock_id: &str, qty: i64) -> NewPaperOrder {
    NewPaperOrder {
        account_id: ACCOUNT_ID,
        stock_id: stock_id.to_string(),
        side: PaperOrderSide::Buy,
        qty,
        note_version_id: NOTE_VERSION_ID,
    }
}

fn order(id: Uuid, side: PaperOrderSide, day: u32, hour: u32) -> PaperOrder {
    PaperOrder {
        id,
        account_id: ACCOUNT_ID,
        stock_id: STOCK_ID.to_string(),
        side,
        qty: 100,
        note_version_id: NOTE_VERSION_ID,
        ordered_at: date(day)
            .and_hms_opt(hour, 0, 0)
            .expect("valid fixture time")
            .and_utc()
            .fixed_offset(),
    }
}

fn bar(day: u32, open: i64) -> Bar {
    let open = Decimal::from(open);
    Bar {
        instrument_id: STOCK_ID.to_string(),
        timeframe: Timeframe::Daily,
        timestamp: date(day).and_time(NaiveTime::MIN).and_utc(),
        open,
        high: open,
        low: open,
        close: open,
        volume: 100,
    }
}

fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, day).unwrap_or_default()
}

fn decided_at(day: u32) -> DateTime<FixedOffset> {
    date(day)
        .and_hms_opt(12, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .fixed_offset()
}

#[tokio::test]
async fn fills_at_the_first_bar_after_the_japan_order_date_using_its_open() {
    let repository = Arc::new(FakePaperTradeRepository::new(
        account(10_000),
        vec![order(FIRST_ORDER_ID, PaperOrderSide::Buy, 5, 16)],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(6, 40), bar(7, 50)]).await;
    bars.seed_ingested_dates(HashSet::from([date(7)])).await;

    let (use_cases, unit_of_work) = use_cases_with_unit_of_work(repository.clone(), bars);
    let outcome = fill_and_snapshot(&use_cases, &repository, decided_at(7)).await;
    let committed = unit_of_work.committed.lock().await.clone();
    let result_transactions = repository.result_transaction_ids().await;
    let commit_state = (
        committed.len(),
        result_transactions.len(),
        result_transactions
            .iter()
            .map(|transaction_id| committed.contains(transaction_id))
            .collect::<Vec<_>>(),
    );

    assert_eq!(
        (outcome, commit_state),
        (
            (
                PaperTradeFillStats {
                    filled: 1,
                    rejected: 0,
                },
                vec![Some(PaperOrderResult::Filled {
                    order_id: FIRST_ORDER_ID,
                    fill_date: date(7),
                    fill_price: Decimal::from(50),
                    decided_at: decided_at(7),
                })],
            ),
            (1, 1, vec![true]),
        ),
    );
}

#[tokio::test]
async fn create_account_trims_fields_before_persisting() {
    let repository = Arc::new(FakePaperTradeRepository::empty());
    let (use_cases, unit_of_work) =
        use_cases_with_unit_of_work(repository.clone(), Arc::new(FakeBarsRepository::new()));

    let result = use_cases
        .create_account(new_account(
            "  Demo account  ",
            "  demo-purpose  ",
            10_000,
            None,
        ))
        .await
        .map_err(|error| error.to_string());
    let persisted_accounts = repository.accounts().await;
    let committed_count = unit_of_work.committed.lock().await.len();
    let expected_account = PaperAccount {
        id: ACCOUNT_ID,
        name: "Demo account".to_string(),
        strategy_id: Uuid::from_u128(6),
        purpose: "demo-purpose".to_string(),
        initial_cash_jpy: Decimal::from(10_000),
        benchmark_stock_id: None,
        started_on: date(1),
    };

    assert_eq!(
        (result, persisted_accounts, committed_count),
        (Ok(expected_account.clone()), vec![expected_account], 1),
    );
}

#[rstest::rstest]
#[case::empty_name("", "demo-purpose", 10_000, None, "name must not be empty")]
#[case::empty_purpose("Demo account", "  ", 10_000, None, "purpose must not be empty")]
#[case::non_positive_cash(
    "Demo account",
    "demo-purpose",
    0,
    None,
    "initial_cash_jpy must be greater than zero"
)]
#[case::foreign_benchmark(
    "Demo account",
    "demo-purpose",
    10_000,
    Some("US:DEMO-A"),
    "benchmark_stock_id must identify a Japanese stock"
)]
#[tokio::test]
async fn create_account_rejects_invalid_input_without_writing(
    #[case] name: &str,
    #[case] purpose: &str,
    #[case] initial_cash_jpy: i64,
    #[case] benchmark_stock_id: Option<&str>,
    #[case] expected_error: &str,
) {
    let repository = Arc::new(FakePaperTradeRepository::empty());
    let (use_cases, unit_of_work) =
        use_cases_with_unit_of_work(repository.clone(), Arc::new(FakeBarsRepository::new()));

    let result = use_cases
        .create_account(new_account(
            name,
            purpose,
            initial_cash_jpy,
            benchmark_stock_id,
        ))
        .await
        .map(|_| ())
        .map_err(|error| error.to_string());
    let outcome = (
        result,
        repository.accounts().await,
        unit_of_work.committed.lock().await.len(),
    );

    assert_eq!(outcome, (Err(expected_error.to_string()), Vec::new(), 0),);
}

#[tokio::test]
async fn record_order_persists_a_valid_japanese_lot_order() {
    let repository = Arc::new(FakePaperTradeRepository::new(account(10_000), Vec::new()));
    let (use_cases, unit_of_work) =
        use_cases_with_unit_of_work(repository.clone(), Arc::new(FakeBarsRepository::new()));
    let new_order = new_order(STOCK_ID, 100);

    let result = use_cases
        .record_order(new_order.clone())
        .await
        .map_err(|error| error.to_string());
    let inserted_orders = repository.inserted_orders().await;
    let committed_count = unit_of_work.committed.lock().await.len();
    let expected_order = PaperOrder {
        id: FIRST_ORDER_ID,
        account_id: ACCOUNT_ID,
        stock_id: STOCK_ID.to_string(),
        side: PaperOrderSide::Buy,
        qty: 100,
        note_version_id: NOTE_VERSION_ID,
        ordered_at: decided_at(5),
    };

    assert_eq!(
        (result, inserted_orders, committed_count),
        (Ok(expected_order), vec![new_order], 1),
    );
}

#[tokio::test]
async fn record_order_does_not_write_when_the_account_is_missing() {
    let repository = Arc::new(FakePaperTradeRepository::empty());
    let (use_cases, unit_of_work) =
        use_cases_with_unit_of_work(repository.clone(), Arc::new(FakeBarsRepository::new()));

    let result = use_cases
        .record_order(new_order(STOCK_ID, 100))
        .await
        .map(|_| ())
        .map_err(|error| error.to_string());
    let outcome = (
        result,
        repository.inserted_orders().await,
        unit_of_work.committed.lock().await.len(),
    );

    assert_eq!(
        outcome,
        (
            Err(format!("paper account {ACCOUNT_ID} not found")),
            Vec::<NewPaperOrder>::new(),
            0,
        ),
    );
}

#[tokio::test]
async fn same_day_orders_use_ordered_at_for_cash_availability() {
    let repository = Arc::new(FakePaperTradeRepository::new(
        account(5_000),
        vec![
            order(FIRST_ORDER_ID, PaperOrderSide::Buy, 5, 1),
            order(SECOND_ORDER_ID, PaperOrderSide::Buy, 5, 2),
        ],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(6, 50)]).await;
    bars.seed_ingested_dates(HashSet::from([date(6)])).await;

    let use_cases = use_cases(repository.clone(), bars);
    let result = fill_and_snapshot(&use_cases, &repository, decided_at(6)).await;

    assert_eq!(
        result,
        (
            PaperTradeFillStats {
                filled: 1,
                rejected: 1,
            },
            vec![
                Some(PaperOrderResult::Filled {
                    order_id: FIRST_ORDER_ID,
                    fill_date: date(6),
                    fill_price: Decimal::from(50),
                    decided_at: decided_at(6),
                }),
                Some(PaperOrderResult::Rejected {
                    order_id: SECOND_ORDER_ID,
                    reject_reason: PaperOrderRejectReason::InsufficientCash,
                    decided_at: decided_at(6),
                }),
            ],
        ),
    );
}

#[tokio::test]
async fn filled_sales_restore_cash_and_shares_before_later_orders() {
    let repository = Arc::new(FakePaperTradeRepository::new(
        account(10_000),
        vec![
            order(FIRST_ORDER_ID, PaperOrderSide::Buy, 5, 1),
            order(SECOND_ORDER_ID, PaperOrderSide::Sell, 6, 1),
            order(THIRD_ORDER_ID, PaperOrderSide::Buy, 7, 1),
        ],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(6, 100), bar(7, 150), bar(8, 150)])
        .await;
    bars.seed_ingested_dates(HashSet::from([date(6), date(7), date(8)]))
        .await;

    let use_cases = use_cases(repository.clone(), bars);
    let result = fill_and_snapshot(&use_cases, &repository, decided_at(8)).await;

    assert_eq!(
        result,
        (
            PaperTradeFillStats {
                filled: 3,
                rejected: 0,
            },
            vec![
                Some(PaperOrderResult::Filled {
                    order_id: FIRST_ORDER_ID,
                    fill_date: date(6),
                    fill_price: Decimal::from(100),
                    decided_at: decided_at(8),
                }),
                Some(PaperOrderResult::Filled {
                    order_id: SECOND_ORDER_ID,
                    fill_date: date(7),
                    fill_price: Decimal::from(150),
                    decided_at: decided_at(8),
                }),
                Some(PaperOrderResult::Filled {
                    order_id: THIRD_ORDER_ID,
                    fill_date: date(8),
                    fill_price: Decimal::from(150),
                    decided_at: decided_at(8),
                }),
            ],
        ),
    );
}

#[tokio::test]
async fn a_saved_buy_fill_restores_shares_for_a_later_pending_sale() {
    let buy_order = order(FIRST_ORDER_ID, PaperOrderSide::Buy, 5, 1);
    let repository = Arc::new(FakePaperTradeRepository::with_results(
        account(10_000),
        vec![
            PaperOrderWithResult {
                order: buy_order,
                result: Some(PaperOrderResult::Filled {
                    order_id: FIRST_ORDER_ID,
                    fill_date: date(6),
                    fill_price: Decimal::from(50),
                    decided_at: decided_at(6),
                }),
            },
            PaperOrderWithResult {
                order: order(SECOND_ORDER_ID, PaperOrderSide::Sell, 6, 10),
                result: None,
            },
        ],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(7, 80)]).await;
    bars.seed_ingested_dates(HashSet::from([date(7)])).await;

    let use_cases = use_cases(repository.clone(), bars);
    let result = fill_and_snapshot(&use_cases, &repository, decided_at(7)).await;

    assert_eq!(
        result,
        (
            PaperTradeFillStats {
                filled: 1,
                rejected: 0,
            },
            vec![
                Some(PaperOrderResult::Filled {
                    order_id: FIRST_ORDER_ID,
                    fill_date: date(6),
                    fill_price: Decimal::from(50),
                    decided_at: decided_at(6),
                }),
                Some(PaperOrderResult::Filled {
                    order_id: SECOND_ORDER_ID,
                    fill_date: date(7),
                    fill_price: Decimal::from(80),
                    decided_at: decided_at(7),
                }),
            ],
        ),
    );
}

#[tokio::test]
async fn rejects_sales_without_enough_filled_shares() {
    let repository = Arc::new(FakePaperTradeRepository::new(
        account(100_000),
        vec![order(FIRST_ORDER_ID, PaperOrderSide::Sell, 5, 1)],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(6, 50)]).await;
    bars.seed_ingested_dates(HashSet::from([date(6)])).await;

    let use_cases = use_cases(repository.clone(), bars);
    let result = fill_and_snapshot(&use_cases, &repository, decided_at(6)).await;

    assert_eq!(
        result,
        (
            PaperTradeFillStats {
                filled: 0,
                rejected: 1,
            },
            vec![Some(PaperOrderResult::Rejected {
                order_id: FIRST_ORDER_ID,
                reject_reason: PaperOrderRejectReason::InsufficientShares,
                decided_at: decided_at(6),
            })],
        ),
    );
}

#[rstest::rstest]
#[case::four_sessions_keep_the_order_pending(4, 9, vec![None])]
#[case::fifth_session_rejects_the_order(5, 12, vec![Some(PaperOrderResult::Rejected { order_id: FIRST_ORDER_ID, reject_reason: PaperOrderRejectReason::DailyBarUnavailable, decided_at: decided_at(12) })])]
#[tokio::test]
async fn order_without_a_later_daily_bar_waits_for_five_completed_sessions(
    #[case] session_count: u32,
    #[case] as_of_day: u32,
    #[case] expected_results: Vec<Option<PaperOrderResult>>,
) {
    let repository = Arc::new(FakePaperTradeRepository::new(
        account(100_000),
        vec![order(FIRST_ORDER_ID, PaperOrderSide::Buy, 5, 1)],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(5, 50)]).await;
    bars.seed_ingested_dates(
        [6, 7, 8, 9, 12]
            .into_iter()
            .take(session_count as usize)
            .map(date)
            .collect(),
    )
    .await;

    let use_cases = use_cases(repository.clone(), bars);
    let result = fill_and_snapshot(&use_cases, &repository, decided_at(as_of_day)).await;
    let expected_stats = if session_count == 5 {
        PaperTradeFillStats {
            filled: 0,
            rejected: 1,
        }
    } else {
        PaperTradeFillStats::default()
    };

    assert_eq!(result, (expected_stats, expected_results));
}

#[tokio::test]
async fn rejects_a_bar_that_first_appears_after_the_fifth_session() {
    let repository = Arc::new(FakePaperTradeRepository::new(
        account(100_000),
        vec![order(FIRST_ORDER_ID, PaperOrderSide::Buy, 5, 1)],
    ));
    let bars = Arc::new(FakeBarsRepository::new());
    bars.seed_bars(vec![bar(13, 50)]).await;
    bars.seed_ingested_dates(HashSet::from([
        date(6),
        date(7),
        date(8),
        date(9),
        date(12),
    ]))
    .await;

    let use_cases = use_cases(repository.clone(), bars);
    let result = fill_and_snapshot(&use_cases, &repository, decided_at(13)).await;

    assert_eq!(
        result,
        (
            PaperTradeFillStats {
                filled: 0,
                rejected: 1,
            },
            vec![Some(PaperOrderResult::Rejected {
                order_id: FIRST_ORDER_ID,
                reject_reason: PaperOrderRejectReason::DailyBarUnavailable,
                decided_at: decided_at(13),
            })],
        ),
    );
}

#[rstest::rstest]
#[case::foreign_stock("US:DEMO-A", 100, "paper orders only support Japanese stocks")]
#[case::non_lot_quantity(STOCK_ID, 101, "qty must be a positive multiple of 100")]
#[tokio::test]
async fn record_order_rejects_invalid_input_without_writing(
    #[case] stock_id: &str,
    #[case] qty: i64,
    #[case] expected_error: &str,
) {
    let repository = Arc::new(FakePaperTradeRepository::new(account(10_000), Vec::new()));
    let use_cases = use_cases(repository.clone(), Arc::new(FakeBarsRepository::new()));
    let result = use_cases.record_order(new_order(stock_id, qty)).await;
    let outcome = (
        result.map(|_| ()).map_err(|error| error.to_string()),
        repository.inserted_orders().await,
    );

    assert_eq!(
        outcome,
        (Err(expected_error.to_string()), Vec::<NewPaperOrder>::new(),),
    );
}
