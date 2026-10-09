use async_trait::async_trait;
use core_application::paper_trade::{
    NewPaperAccount, NewPaperOrder, PaperAccount, PaperOrder, PaperOrderOutcome,
    PaperOrderRejectReason, PaperOrderResult, PaperOrderSide, PaperOrderWithResult,
    PaperTradeRepository, PaperTradeRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::LockType;
use sea_orm::{EntityTrait, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::entities::{paper_account, paper_order, paper_order_result};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone, Default)]
pub struct PostgresPaperTradeRepository;

impl PostgresPaperTradeRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PaperTradeRepository for PostgresPaperTradeRepository {
    async fn list_accounts(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        paper_account::Entity::find()
            .order_by_asc(paper_account::Column::Name)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(to_account).collect())
            .map_err(repository_error)
    }

    async fn insert_account(
        &self,
        transaction: &UnitOfWorkTransaction,
        account: NewPaperAccount,
    ) -> Result<PaperAccount, PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        paper_account::Entity::insert(paper_account::ActiveModel {
            id: NotSet,
            name: Set(account.name),
            strategy_id: Set(account.strategy_id),
            purpose: Set(account.purpose),
            initial_cash_jpy: Set(account.initial_cash_jpy),
            benchmark_stock_id: Set(account.benchmark_stock_id),
            started_on: Set(account.started_on),
            created_at: NotSet,
            updated_at: NotSet,
        })
        .exec_with_returning(transaction)
        .await
        .map(to_account)
        .map_err(repository_error)
    }

    async fn insert_order(
        &self,
        transaction: &UnitOfWorkTransaction,
        order: NewPaperOrder,
    ) -> Result<PaperOrder, PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        paper_order::Entity::insert(paper_order::ActiveModel {
            id: NotSet,
            account_id: Set(order.account_id),
            stock_id: Set(order.stock_id),
            side: Set(order.side.as_str().to_string()),
            qty: Set(order.qty),
            note_version_id: Set(order.note_version_id),
            ordered_at: NotSet,
        })
        .exec_with_returning(transaction)
        .await
        .map_err(repository_error)
        .and_then(to_order)
    }

    async fn lock_accounts_for_filling(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        paper_account::Entity::find()
            .lock(LockType::Update)
            .all(transaction)
            .await
            .map(|rows| rows.into_iter().map(to_account).collect())
            .map_err(repository_error)
    }

    async fn list_orders_for_filling(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperOrderWithResult>, PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        let orders = paper_order::Entity::find()
            .all(transaction)
            .await
            .map_err(repository_error)?;
        let results = paper_order_result::Entity::find()
            .all(transaction)
            .await
            .map_err(repository_error)?;
        let results = results
            .into_iter()
            .map(|row| {
                let result = to_result(row)?;
                Ok((result.order_id(), result))
            })
            .collect::<Result<std::collections::HashMap<_, _>, PaperTradeRepositoryError>>()?;

        orders
            .into_iter()
            .map(|row| {
                let order = to_order(row)?;
                Ok(PaperOrderWithResult {
                    result: results.get(&order.id).cloned(),
                    order,
                })
            })
            .collect()
    }

    async fn insert_result(
        &self,
        transaction: &UnitOfWorkTransaction,
        result: PaperOrderResult,
    ) -> Result<(), PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        let outcome = result.outcome();
        let active_model = match result {
            PaperOrderResult::Filled {
                order_id,
                fill_date,
                fill_price,
                decided_at,
            } => paper_order_result::ActiveModel {
                order_id: Set(order_id),
                outcome: Set(outcome.as_str().to_string()),
                fill_date: Set(Some(fill_date)),
                fill_price: Set(Some(fill_price)),
                reject_reason: Set(None),
                decided_at: Set(decided_at),
            },
            PaperOrderResult::Rejected {
                order_id,
                reject_reason,
                decided_at,
            } => paper_order_result::ActiveModel {
                order_id: Set(order_id),
                outcome: Set(outcome.as_str().to_string()),
                fill_date: Set(None),
                fill_price: Set(None),
                reject_reason: Set(Some(reject_reason.as_str().to_string())),
                decided_at: Set(decided_at),
            },
        };
        paper_order_result::Entity::insert(active_model)
            .exec_without_returning(transaction)
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    async fn account_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        account_id: Uuid,
    ) -> Result<bool, PaperTradeRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(PaperTradeRepositoryError::InvalidTransaction)?;
        paper_account::Entity::find_by_id(account_id)
            .one(transaction)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }
}

fn to_account(model: paper_account::Model) -> PaperAccount {
    PaperAccount {
        id: model.id,
        name: model.name,
        strategy_id: model.strategy_id,
        purpose: model.purpose,
        initial_cash_jpy: model.initial_cash_jpy,
        benchmark_stock_id: model.benchmark_stock_id,
        started_on: model.started_on,
    }
}

fn to_order(model: paper_order::Model) -> Result<PaperOrder, PaperTradeRepositoryError> {
    let side = PaperOrderSide::parse(&model.side).ok_or_else(|| {
        PaperTradeRepositoryError::InvalidData(format!(
            "paper_order.side has unsupported value: {}",
            model.side
        ))
    })?;
    Ok(PaperOrder {
        id: model.id,
        account_id: model.account_id,
        stock_id: model.stock_id,
        side,
        qty: model.qty,
        note_version_id: model.note_version_id,
        ordered_at: model.ordered_at,
    })
}

fn to_result(
    model: paper_order_result::Model,
) -> Result<PaperOrderResult, PaperTradeRepositoryError> {
    let invalid = |field: &str| {
        PaperTradeRepositoryError::InvalidData(format!(
            "paper_order_result {} is inconsistent",
            field
        ))
    };
    match PaperOrderOutcome::parse(&model.outcome) {
        Some(PaperOrderOutcome::Filled) => {
            match (model.fill_date, model.fill_price, model.reject_reason) {
                (Some(fill_date), Some(fill_price), None) => Ok(PaperOrderResult::Filled {
                    order_id: model.order_id,
                    fill_date,
                    fill_price,
                    decided_at: model.decided_at,
                }),
                _ => Err(invalid("fill columns")),
            }
        }
        Some(PaperOrderOutcome::Rejected) => {
            match (model.fill_date, model.fill_price, model.reject_reason) {
                (None, None, Some(reason)) => Ok(PaperOrderResult::Rejected {
                    order_id: model.order_id,
                    reject_reason: parse_reject_reason(&reason)?,
                    decided_at: model.decided_at,
                }),
                _ => Err(invalid("reject columns")),
            }
        }
        None => Err(invalid("outcome")),
    }
}

fn parse_reject_reason(reason: &str) -> Result<PaperOrderRejectReason, PaperTradeRepositoryError> {
    PaperOrderRejectReason::parse(reason).ok_or_else(|| {
        PaperTradeRepositoryError::InvalidData(format!("unsupported reject reason: {reason}"))
    })
}

fn repository_error(error: sea_orm::DbErr) -> PaperTradeRepositoryError {
    PaperTradeRepositoryError::Database(persistence_error(error))
}
