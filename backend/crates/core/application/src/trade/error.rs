use super::repository::TradeRepositoryError;
use crate::change_history::ChangeHistoryError;
use crate::note::NoteRepositoryError;
use crate::strategy_existence::StrategyExistenceError;
use crate::unit_of_work::UnitOfWorkError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TradeUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("trade {0} not found")]
    NotFound(Uuid),
    #[error("{0}")]
    ResourceNotFound(String),
    #[error(transparent)]
    Repository(#[from] TradeRepositoryError),
    #[error(transparent)]
    NoteRepository(#[from] NoteRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
    #[error(transparent)]
    StrategyExistence(#[from] StrategyExistenceError),
}
