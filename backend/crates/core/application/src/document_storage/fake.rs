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
        let document_id = snapshots
            .iter()
            .find(|snapshot| {
                snapshot.document.metadata.source == metadata.source
                    && snapshot.document.metadata.source_document_id == metadata.source_document_id
            })
            .map(|snapshot| snapshot.document.id);
        let document_id = document_id.unwrap_or_else(Uuid::new_v4);
        for snapshot in snapshots
            .iter_mut()
            .filter(|snapshot| snapshot.document.id == document_id)
        {
            snapshot.document.metadata = metadata.clone();
        }
        let latest_index = snapshots
            .iter()
            .enumerate()
            .filter(|(_, snapshot)| snapshot.document.id == document_id)
            .max_by_key(|(index, snapshot)| (snapshot.version.retrieved_at, *index))
            .map(|(index, _)| index);
        if let Some(index) = latest_index
            && snapshots[index].version.content_hash == content_hash
        {
            return Ok(snapshots[index].clone());
        }
        let version = DocumentVersion {
            id: Uuid::new_v4(),
            document_id,
            content_hash,
            retrieved_at,
            body,
        };
        let snapshot = DocumentSnapshot {
            document: Document {
                id: document_id,
                metadata,
            },
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
        snapshots.push(snapshot.clone());
        *self.version_count.lock().await += 1;
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
            .enumerate()
            .filter(|snapshot| {
                snapshot.1.document.metadata.source == source
                    && snapshot.1.document.metadata.source_document_id == source_document_id
                    && snapshot.1.version.retrieved_at <= as_of
            })
            .max_by_key(|(index, snapshot)| (snapshot.version.retrieved_at, *index))
            .map(|(_, snapshot)| snapshot.clone()))
    }
}
