use thiserror::Error;

use crate::change_history::ChangeHistoryError;
use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum CommentRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[derive(Debug, Error)]
pub enum CommentUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Repository(#[from] CommentRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
