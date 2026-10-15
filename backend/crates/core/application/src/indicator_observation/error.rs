use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum IndicatorObservationRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[derive(Debug, Error)]
pub enum IndicatorObservationUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repository(#[from] IndicatorObservationRepositoryError),
}
