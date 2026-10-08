use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{
    NewPaperAccount, NewPaperOrder, PaperAccount, PaperOrder, PaperOrderResult,
    PaperOrderWithResult,
};

#[derive(Debug, Error)]
pub enum PaperTradeRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("stored paper trade data is invalid: {0}")]
    InvalidData(String),
}

#[async_trait]
pub trait PaperTradeRepository: Send + Sync {
    async fn insert_account(
        &self,
        transaction: &UnitOfWorkTransaction,
        account: NewPaperAccount,
    ) -> Result<PaperAccount, PaperTradeRepositoryError>;

    async fn insert_order(
        &self,
        transaction: &UnitOfWorkTransaction,
        order: NewPaperOrder,
    ) -> Result<PaperOrder, PaperTradeRepositoryError>;

    async fn lock_accounts_for_filling(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperAccount>, PaperTradeRepositoryError>;

    async fn list_orders_for_filling(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<PaperOrderWithResult>, PaperTradeRepositoryError>;

    async fn insert_result(
        &self,
        transaction: &UnitOfWorkTransaction,
        result: PaperOrderResult,
    ) -> Result<(), PaperTradeRepositoryError>;

    async fn account_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        account_id: Uuid,
    ) -> Result<bool, PaperTradeRepositoryError>;
}

pub type SharedPaperTradeRepository = Arc<dyn PaperTradeRepository + Send + Sync>;
