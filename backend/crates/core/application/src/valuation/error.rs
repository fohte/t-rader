use thiserror::Error;

use super::repository::ValuationRepositoryError;

#[derive(Debug, Error)]
pub enum ValuationUseCaseError {
    #[error(transparent)]
    Repository(#[from] ValuationRepositoryError),
}
