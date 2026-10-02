use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum AccountRiskPolicyDataError {
    #[error("{0}")]
    Validation(String),
    #[error("invalid risk_policy: {0}")]
    InvalidData(serde_json::Error),
}

#[derive(Debug, Error)]
pub enum AccountRiskPolicyRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}
