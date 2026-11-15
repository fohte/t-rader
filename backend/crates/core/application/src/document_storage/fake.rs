use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{DocumentStorageRepository, DocumentStorageRepositoryError};
use super::types::{
    Document, DocumentChunk, DocumentChunkInput, DocumentMetadata, DocumentSnapshot,
    DocumentVersion,
};

#[derive(Default)]
pub struct FakeDocumentStorageRepository {
    snapshots: Mutex<Vec<DocumentSnapshot>>,
    version_count: Mutex<usize>,
}

impl FakeDocumentStorageRepository {
    pub async fn version_count(&self) -> usize {
        *self.version_count.lock().await
    }

    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), DocumentStorageRepositoryError> {
        transaction
            .downcast_ref::<FakeTransaction>()
            .map(|_| ())
            .ok_or(DocumentStorageRepositoryError::InvalidTransaction)
    }
}

#[async_trait]
impl DocumentStorageRepository for FakeDocumentStorageRepository {
    async fn save(
        &self,
        transaction: &UnitOfWorkTransaction,
        metadata: DocumentMetadata,
        content_hash: String,
        retrieved_at: DateTime<FixedOffset>,
        body: String,
        chunks: Vec<DocumentChunkInput>,
    ) -> Result<DocumentSnapshot, DocumentStorageRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut snapshots = self.snapshots.lock().await;
        let document = snapshots
            .iter()
            .find(|snapshot| {
                snapshot.document.metadata.source == metadata.source
                    && snapshot.document.metadata.source_document_id == metadata.source_document_id
            })
            .map(|snapshot| Document {
                id: snapshot.document.id,
                metadata: metadata.clone(),
            })
            .unwrap_or_else(|| Document {
                id: Uuid::new_v4(),
                metadata: metadata.clone(),
            });
        let existing = snapshots.iter().position(|snapshot| {
            snapshot.document.id == document.id && snapshot.version.content_hash == content_hash
        });
        let version = existing
            .map(|index| snapshots[index].version.clone())
            .unwrap_or_else(|| DocumentVersion {
                id: Uuid::new_v4(),
                document_id: document.id,
                content_hash,
                retrieved_at,
                body,
            });
        let snapshot = DocumentSnapshot {
            document,
            version: version.clone(),
            chunks: chunks
                .into_iter()
                .map(|chunk| DocumentChunk {
                    id: Uuid::new_v4(),
                    version_id: version.id,
                    sequence: chunk.sequence,
                    start_offset: chunk.start_offset,
                    end_offset: chunk.end_offset,
                    body: chunk.body,
                })
                .collect(),
        };
        if let Some(index) = existing {
            snapshots[index] = snapshot.clone();
        } else {
            snapshots.push(snapshot.clone());
            *self.version_count.lock().await += 1;
        }
        Ok(snapshot)
    }

    async fn find_latest_as_of(
        &self,
        source: &str,
        source_document_id: &str,
        as_of: DateTime<FixedOffset>,
    ) -> Result<Option<DocumentSnapshot>, DocumentStorageRepositoryError> {
        let snapshots = self.snapshots.lock().await;
        Ok(snapshots
            .iter()
            .filter(|snapshot| {
                snapshot.document.metadata.source == source
                    && snapshot.document.metadata.source_document_id == source_document_id
                    && snapshot.version.retrieved_at <= as_of
            })
            .max_by_key(|snapshot| (snapshot.version.retrieved_at, snapshot.version.id))
            .cloned())
    }
}
