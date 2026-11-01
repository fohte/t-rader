use thiserror::Error;

use crate::change_history::ChangeHistoryError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::NoteTagRepositoryError;

#[derive(Debug, Error)]
pub enum NoteTagUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] NoteTagRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
