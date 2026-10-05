use std::sync::Arc;

use async_trait::async_trait;
use core_domain::us_stock_master::UsStockMasterEntry;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Debug, Error)]
pub enum UsStockMasterRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait UsStockMasterRepository: Send + Sync {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        entries: &[UsStockMasterEntry],
    ) -> Result<usize, UsStockMasterRepositoryError>;
}

pub type SharedUsStockMasterRepository = Arc<dyn UsStockMasterRepository>;
