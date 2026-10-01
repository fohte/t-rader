use thiserror::Error;

use crate::unit_of_work::UnitOfWorkError;

use super::repository::GroupAxisRepositoryError;

#[derive(Debug, Error)]
pub enum GroupAxisUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] GroupAxisRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
