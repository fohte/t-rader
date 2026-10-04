use thiserror::Error;

use crate::refs::RefUseCaseError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum StrategyEarningsTargetRepositoryError {
    #[error(transparent)]
    Database(#[from] crate::persistence::PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[derive(Debug, Error)]
pub enum StrategyEarningsTargetUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{ref_kind} reference not found: {ref_id}")]
    ReferenceNotFound { ref_kind: String, ref_id: String },
    #[error(transparent)]
    Reference(#[from] RefUseCaseError),
    #[error(transparent)]
    Repository(#[from] StrategyEarningsTargetRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
