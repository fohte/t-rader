use thiserror::Error;
use uuid::Uuid;

use super::repository::TradeRepositoryError;
use crate::change_history::ChangeHistoryError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum TradeUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("trade {0} not found")]
    NotFound(Uuid),
    #[error(transparent)]
    Repository(#[from] TradeRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
