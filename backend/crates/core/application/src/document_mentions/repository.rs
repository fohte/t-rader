use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;

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
}

#[async_trait]
pub trait DocumentMentionRepository: Send + Sync {
    async fn find_registered_stocks(
        &self,
    ) -> Result<Vec<RegisteredStock>, DocumentMentionRepositoryError>;

    async fn save_mentions(
        &self,
        mentions: Vec<DocumentMention>,
    ) -> Result<(), DocumentMentionRepositoryError>;
}
