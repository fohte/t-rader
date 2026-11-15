use thiserror::Error;

use super::repository::DocumentStorageRepositoryError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum DocumentStorageUseCaseError {
    #[error(transparent)]
    Repository(#[from] DocumentStorageRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error("invalid document chunk at sequence {sequence}: {reason}")]
    InvalidChunk { sequence: i32, reason: &'static str },
    #[error("document metadata field '{0}' must not be empty")]
    EmptyMetadata(&'static str),
}
