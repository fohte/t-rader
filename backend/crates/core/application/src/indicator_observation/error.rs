use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum IndicatorObservationRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

#[derive(Debug, Error)]
pub enum IndicatorObservationUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repository(#[from] IndicatorObservationRepositoryError),
}
