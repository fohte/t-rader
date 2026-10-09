use thiserror::Error;
use uuid::Uuid;

use crate::{bars::BarsRepositoryError, unit_of_work::UnitOfWorkError};

use super::repository::PaperTradeRepositoryError;

#[derive(Debug, Error)]
pub enum PaperTradeUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("paper account {0} not found")]
    AccountNotFound(Uuid),
    #[error("latest daily bar for stock {0} is unavailable")]
    LatestDailyBarUnavailable(String),
    #[error(transparent)]
    Repository(#[from] PaperTradeRepositoryError),
    #[error(transparent)]
    BarsRepository(#[from] BarsRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
