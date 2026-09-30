use thiserror::Error;
use uuid::Uuid;

use crate::strategy::StrategyRepositoryError;
use crate::strategy_existence::StrategyExistenceError;
use crate::strategy_task::SubmitTaskError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::TriggerRepositoryError;

#[derive(Debug, Error)]
pub enum TriggerUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("trigger {0} not found")]
    NotFound(Uuid),
    #[error("hook {0} not found")]
    HookNotFound(String),
    #[error("trigger {0} is disabled")]
    Disabled(Uuid),
    #[error("trigger {0} has no strategy_id")]
    NoStrategy(Uuid),
    #[error(transparent)]
    Submit(#[from] SubmitTaskError),
    #[error(transparent)]
    Repository(#[from] TriggerRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    StrategyExistence(#[from] StrategyExistenceError),
    #[error(transparent)]
    StrategyRepository(#[from] StrategyRepositoryError),
}
