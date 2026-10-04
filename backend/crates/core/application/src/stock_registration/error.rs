use thiserror::Error;

use crate::change_history::ChangeHistoryError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::StockRegistrationRepositoryError;

#[derive(Debug, Error)]
pub enum StockRegistrationUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repository(#[from] StockRegistrationRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] ChangeHistoryError),
}
