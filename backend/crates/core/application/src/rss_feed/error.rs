use thiserror::Error;
use uuid::Uuid;

use super::repository::RssFeedRepositoryError;

#[derive(Debug, Error)]
pub enum RssFeedUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("rss feed {0} not found")]
    NotFound(Uuid),
    #[error(transparent)]
    Repository(#[from] RssFeedRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] crate::unit_of_work::UnitOfWorkError),
}
