use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{DocumentChunkInput, DocumentMetadata, DocumentSnapshot};

#[derive(Debug, Error)]
pub enum DocumentStorageRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait DocumentStorageRepository: Send + Sync {
    async fn save(
        &self,
        transaction: &UnitOfWorkTransaction,
        metadata: DocumentMetadata,
        content_hash: String,
        retrieved_at: DateTime<FixedOffset>,
        body: String,
        chunks: Vec<DocumentChunkInput>,
    ) -> Result<DocumentSnapshot, DocumentStorageRepositoryError>;

    async fn find_latest_as_of(
        &self,
        source: &str,
        source_document_id: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<Option<DocumentSnapshot>, DocumentStorageRepositoryError>;
}

pub type SharedDocumentStorageRepository =
    std::sync::Arc<dyn DocumentStorageRepository + Send + Sync>;
