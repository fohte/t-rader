use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("{0}")]
    Database(String),
    #[error("{0}")]
    MissingReference(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    ConstraintViolation(String),
    #[error("{0}")]
    RecordNotUpdated(String),
}
