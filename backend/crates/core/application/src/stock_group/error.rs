use thiserror::Error;

use crate::change_history::ChangeHistoryError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::StockGroupRepositoryError;

#[derive(Debug, Error)]
pub enum StockGroupUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("group axis {0} not found")]
    AxisNotFound(String),
    #[error("stock group {axis_key}/{group_key} not found")]
    GroupNotFound { axis_key: String, group_key: String },
    #[error(transparent)]
    Repository(#[from] StockGroupRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
