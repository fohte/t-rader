use thiserror::Error;

use crate::change_history::ChangeHistoryError;
use crate::note::NoteUseCaseError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::NoteKindRepositoryError;

#[derive(Debug, Error)]
pub enum NoteKindUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] NoteKindRepositoryError),
    #[error(transparent)]
    Note(#[from] NoteUseCaseError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
