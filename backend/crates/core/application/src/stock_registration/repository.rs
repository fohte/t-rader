use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::NewStockRegistration;

#[derive(Debug, Error)]
pub enum StockRegistrationRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait StockRegistrationRepository: Send + Sync {
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock: NewStockRegistration,
    ) -> Result<(), StockRegistrationRepositoryError>;
}

pub type SharedStockRegistrationRepository =
    std::sync::Arc<dyn StockRegistrationRepository + Send + Sync>;
