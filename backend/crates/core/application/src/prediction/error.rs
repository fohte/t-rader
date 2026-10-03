use thiserror::Error;
use uuid::Uuid;

use super::repository::PredictionRepositoryError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum PredictionUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("note {0} not found")]
    NoteNotFound(Uuid),
    #[error(transparent)]
    Repository(#[from] PredictionRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
