use std::sync::Arc;

use thiserror::Error;

use crate::document_storage::DocumentChunk;
use crate::unit_of_work::{SharedUnitOfWork, UnitOfWorkError};

use super::matcher::{ChunkText, InvalidOffset, find_mentions};
use super::{DocumentMention, DocumentMentionRepository, DocumentMentionRepositoryError};

#[derive(Debug, Error)]
pub enum DocumentMentionUseCasesError {
    #[error(transparent)]
    Repository(#[from] DocumentMentionRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error("文書片の byte offset を銘柄 mention の範囲に変換できません")]
    InvalidOffset,
}

#[derive(Clone)]
pub struct DocumentMentionUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: Arc<dyn DocumentMentionRepository>,
}

impl DocumentMentionUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: Arc<dyn DocumentMentionRepository>,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn record(
        &self,
        chunks: &[DocumentChunk],
    ) -> Result<Vec<DocumentMention>, DocumentMentionUseCasesError> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let transaction = self.unit_of_work.begin().await?;
        let chunk_ids = chunks.iter().map(|chunk| chunk.id).collect::<Vec<_>>();
        self.repository
            .lock_chunks_for_update(&transaction, chunk_ids.clone())
            .await?;
        let stocks = self.repository.find_registered_stocks(&transaction).await?;
        let chunk_texts = chunks
            .iter()
            .map(|chunk| ChunkText {
                id: chunk.id,
                start_offset: chunk.start_offset,
                body: &chunk.body,
            })
            .collect::<Vec<_>>();
        let mentions = find_mentions(&chunk_texts, &stocks)
            .map_err(|InvalidOffset| DocumentMentionUseCasesError::InvalidOffset)?;
        self.repository
            .replace_mentions(&transaction, chunk_ids, mentions.clone())
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(mentions)
    }

    pub async fn find_for_chunks(
        &self,
        chunk_ids: Vec<uuid::Uuid>,
    ) -> Result<Vec<DocumentMention>, DocumentMentionUseCasesError> {
        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }
        self.repository
            .find_mentions_for_chunks(chunk_ids)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
