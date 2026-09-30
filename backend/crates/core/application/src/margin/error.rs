use thiserror::Error;

use super::repository::MarginRepositoryError;

#[derive(Debug, Error)]
pub enum MarginUseCaseError {
    #[error("from must be on or before to")]
    InvalidDateRange,
    #[error(transparent)]
    Repository(#[from] MarginRepositoryError),
}
