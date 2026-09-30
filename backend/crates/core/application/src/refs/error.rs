use thiserror::Error;

use crate::unit_of_work::UnitOfWorkError;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum RefRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[derive(Debug, Error)]
pub enum RefUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repository(#[from] RefRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
