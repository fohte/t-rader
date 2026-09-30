use std::sync::Arc;

use async_trait::async_trait;
use core_domain::equity_master::EquityMasterEntry;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Debug, Error)]
pub enum EquityMasterRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait EquityMasterRepository: Send + Sync {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        entries: &[EquityMasterEntry],
    ) -> Result<usize, EquityMasterRepositoryError>;
}

pub type SharedEquityMasterRepository = Arc<dyn EquityMasterRepository>;
