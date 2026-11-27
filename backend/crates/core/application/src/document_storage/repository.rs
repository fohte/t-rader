use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{DocumentChunkContext, DocumentChunkInput, DocumentMetadata, DocumentSnapshot};

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

    async fn find_chunk_context(
        &self,
        chunk_id: uuid::Uuid,
        as_of: Option<DateTime<FixedOffset>>,
    ) -> Result<Option<DocumentChunkContext>, DocumentStorageRepositoryError>;
}

pub type SharedDocumentStorageRepository =
    std::sync::Arc<dyn DocumentStorageRepository + Send + Sync>;
