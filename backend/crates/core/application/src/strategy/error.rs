use thiserror::Error;
use uuid::Uuid;

use crate::change_history::ChangeHistoryError;
use crate::unit_of_work::UnitOfWorkError;

use super::query::StrategySummaryQueryError;
use super::repository::StrategyRepositoryError;

#[derive(Debug, Error)]
pub enum StrategyUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("strategy {0} not found")]
    NotFound(Uuid),
    #[error(transparent)]
    Repository(#[from] StrategyRepositoryError),
    #[error(transparent)]
    SummaryQuery(#[from] StrategySummaryQueryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
