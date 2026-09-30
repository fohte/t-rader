use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum AccountRiskPolicyRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}
