use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveTime};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::{
    bars::FakeBarsRepository,
    paper_trade::{
        NewPaperAccount, NewPaperOrder, PaperAccount, PaperOrder, PaperOrderResult,
        PaperOrderWithResult, PaperTradeRepository, PaperTradeRepositoryError, PaperTradeUseCases,
    },
    unit_of_work::{FakeUnitOfWork, UnitOfWorkTransaction},
};

pub(super) struct FakePaperTradeRepository {
    pub accounts: Vec<PaperAccount>,
    pub orders: Vec<PaperOrderWithResult>,
}

impl FakePaperTradeRepository {
    pub fn new(accounts: Vec<PaperAccount>, orders: Vec<PaperOrderWithResult>) -> Self {
        Self { accounts, orders }
    }
}

#[async_trait]
impl PaperTradeRepository for FakePaperTradeRepository {
    async fn account_for_strategy_purpose(
        &self,
        _transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        purpose: &str,
    ) -> Result<Option<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self
            .accounts
            .iter()
            .find(|account| account.strategy_id == strategy_id && account.purpose == purpose)
            .cloned())
    }

    async fn find_account(
        &self,
        _transaction: &UnitOfWorkTransaction,
        account_id: Uuid,
    ) -> Result<Option<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .cloned())
    }

    async fn list_accounts(
        &self,
        _transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self.accounts.clone())
    }

    async fn list_orders_with_results(
        &self,
        _transaction: &UnitOfWorkTransaction,
        account_id: Option<Uuid>,
    ) -> Result<Vec<PaperOrderWithResult>, PaperTradeRepositoryError> {
        Ok(self
            .orders
            .iter()
            .filter(|item| account_id.is_none_or(|id| item.order.account_id == id))
            .cloned()
            .collect())
    }

    async fn insert_account(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _account: NewPaperAccount,
    ) -> Result<PaperAccount, PaperTradeRepositoryError> {
        Err(PaperTradeRepositoryError::InvalidTransaction)
    }

    async fn insert_order(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _order: NewPaperOrder,
    ) -> Result<PaperOrder, PaperTradeRepositoryError> {
        Err(PaperTradeRepositoryError::InvalidTransaction)
    }

    async fn lock_accounts_for_filling(
        &self,
        _transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError> {
        Ok(self.accounts.clone())
    }

    async fn list_orders_for_filling(
        &self,
        _transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperOrderWithResult>, PaperTradeRepositoryError> {
        Ok(self.orders.clone())
    }

    async fn insert_result(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _result: PaperOrderResult,
    ) -> Result<(), PaperTradeRepositoryError> {
        Err(PaperTradeRepositoryError::InvalidTransaction)
    }

    async fn account_exists(
        &self,
        _transaction: &UnitOfWorkTransaction,
        account_id: Uuid,
    ) -> Result<bool, PaperTradeRepositoryError> {
        Ok(self.accounts.iter().any(|account| account.id == account_id))
    }
}

pub(super) fn use_cases(
    accounts: Vec<PaperAccount>,
    orders: Vec<PaperOrderWithResult>,
    bars: Arc<FakeBarsRepository>,
) -> PaperTradeUseCases {
    PaperTradeUseCases::new(
        Arc::new(FakeUnitOfWork::new()),
        Arc::new(FakePaperTradeRepository::new(accounts, orders)),
        bars,
    )
}

pub(super) fn account(
    id: u128,
    name: &str,
    initial_cash_jpy: i64,
    started_on: NaiveDate,
    benchmark_stock_id: Option<&str>,
) -> PaperAccount {
    PaperAccount {
        id: Uuid::from_u128(id),
        name: name.to_string(),
        strategy_id: Uuid::from_u128(id + 100),
        purpose: format!("demo-purpose-{id}"),
        initial_cash_jpy: Decimal::from(initial_cash_jpy),
        benchmark_stock_id: benchmark_stock_id.map(str::to_string),
        started_on,
    }
}

pub(super) fn order(
    id: u128,
    account_id: Uuid,
    stock_id: &str,
    side: super::PaperOrderSide,
    qty: i64,
    ordered_on: NaiveDate,
) -> PaperOrder {
    PaperOrder {
        id: Uuid::from_u128(id),
        account_id,
        stock_id: stock_id.to_string(),
        side,
        qty,
        note_version_id: Uuid::from_u128(id + 1000),
        ordered_at: at(ordered_on),
    }
}

pub(super) fn filled(
    order: PaperOrder,
    fill_date: NaiveDate,
    fill_price: i64,
) -> PaperOrderWithResult {
    PaperOrderWithResult {
        result: Some(PaperOrderResult::Filled {
            order_id: order.id,
            fill_date,
            fill_price: Decimal::from(fill_price),
            decided_at: at(fill_date),
        }),
        order,
    }
}

pub(super) fn pending(order: PaperOrder) -> PaperOrderWithResult {
    PaperOrderWithResult {
        order,
        result: None,
    }
}

pub(super) fn bar(stock_id: &str, day: u32, open: i64, close: i64) -> Bar {
    let date = date(day);
    Bar {
        instrument_id: stock_id.to_string(),
        timeframe: Timeframe::Daily,
        timestamp: date.and_time(NaiveTime::MIN).and_utc(),
        open: Decimal::from(open),
        high: Decimal::from(open.max(close)),
        low: Decimal::from(open.min(close)),
        close: Decimal::from(close),
        volume: 1_000,
    }
}

pub(super) fn date(day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2031, 1, day).unwrap_or_default()
}

pub(super) fn at(date: NaiveDate) -> DateTime<chrono::FixedOffset> {
    date.and_time(NaiveTime::MIN).and_utc().fixed_offset()
}
