use thiserror::Error;

use super::repository::ShortRatioRepositoryError;

#[derive(Debug, Error)]
pub enum ShortRatioUseCaseError {
    #[error(transparent)]
    Repository(#[from] ShortRatioRepositoryError),
}
