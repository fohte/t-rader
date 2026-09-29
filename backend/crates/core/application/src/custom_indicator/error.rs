use thiserror::Error;
use uuid::Uuid;

use crate::change_history::ChangeHistoryError;
use crate::strategy_existence::StrategyExistenceError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::CustomIndicatorRepositoryError;

#[derive(Debug, Error)]
pub enum CustomIndicatorUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("indicator {0} not found")]
    NotFound(Uuid),
    #[error(transparent)]
    Repository(#[from] CustomIndicatorRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
    #[error(transparent)]
    StrategyExistence(#[from] StrategyExistenceError),
}
