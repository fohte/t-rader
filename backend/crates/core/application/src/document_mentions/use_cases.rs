use std::sync::Arc;

use thiserror::Error;

use crate::document_storage::DocumentChunk;

use super::matcher::{ChunkText, InvalidOffset, find_mentions};
use super::{DocumentMention, DocumentMentionRepository, DocumentMentionRepositoryError};

#[derive(Debug, Error)]
pub enum DocumentMentionUseCasesError {
    #[error(transparent)]
    Repository(#[from] DocumentMentionRepositoryError),
    #[error("文書片の byte offset を銘柄 mention の範囲に変換できません")]
    InvalidOffset,
}

pub struct DocumentMentionUseCases {
    repository: Arc<dyn DocumentMentionRepository>,
}

impl DocumentMentionUseCases {
    pub fn new(repository: Arc<dyn DocumentMentionRepository>) -> Self {
        Self { repository }
    }

    pub async fn record(
        &self,
        chunks: &[DocumentChunk],
    ) -> Result<Vec<DocumentMention>, DocumentMentionUseCasesError> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        let stocks = self.repository.find_registered_stocks().await?;
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
        if !mentions.is_empty() {
            self.repository.save_mentions(mentions.clone()).await?;
        }
        Ok(mentions)
    }
}

#[cfg(test)]
mod tests;
