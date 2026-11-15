use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::DocumentMention;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredStock {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum DocumentMentionRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait DocumentMentionRepository: Send + Sync {
    async fn find_registered_stocks(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<RegisteredStock>, DocumentMentionRepositoryError>;

    async fn replace_mentions(
        &self,
        transaction: &UnitOfWorkTransaction,
        chunk_ids: Vec<uuid::Uuid>,
        mentions: Vec<DocumentMention>,
    ) -> Result<(), DocumentMentionRepositoryError>;
}
