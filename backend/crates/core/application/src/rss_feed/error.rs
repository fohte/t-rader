use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::RssFeedRepositoryError;

#[derive(Debug, Error)]
pub enum RssFeedUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("rss feed with source '{0}' already exists")]
    DuplicateSource(String),
    #[error("rss feed {0} not found")]
    NotFound(Uuid),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}

impl From<RssFeedRepositoryError> for RssFeedUseCaseError {
    fn from(error: RssFeedRepositoryError) -> Self {
        match error {
            RssFeedRepositoryError::Persistence(error) => Self::Persistence(error),
            RssFeedRepositoryError::DuplicateSource(source) => Self::DuplicateSource(source),
            RssFeedRepositoryError::InvalidTransaction => {
                Self::UnitOfWork(UnitOfWorkError::InvalidTransaction)
            }
        }
    }
}
