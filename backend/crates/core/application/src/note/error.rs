use thiserror::Error;

use crate::change_history::ChangeHistoryError;
use crate::note::repository::NoteRepositoryError;
use crate::strategy_existence::StrategyExistenceError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum NoteUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("unknown note kind: {0}")]
    UnknownNoteKind(String),
    #[error("{0}")]
    NotFound(String),
    #[error("note kind {0} not found")]
    ReferencedNoteKindNotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Repository(#[from] NoteRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
    #[error(transparent)]
    StrategyExistence(#[from] StrategyExistenceError),
}
