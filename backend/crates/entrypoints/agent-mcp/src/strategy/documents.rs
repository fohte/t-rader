use std::collections::BTreeMap;

use core_application::document_search::DocumentSearchCriteria;
use core_application::document_storage::{Document, DocumentChunk, DocumentChunkContext};
use rmcp::ErrorData as McpError;
use uuid::Uuid;

use super::dto::{
    DocumentChunkDto, DocumentDto, DocumentMentionDto, DocumentVersionDto,
    DocumentVersionReferenceDto, GetDocumentChunkParams, GetDocumentChunkResult,
    SearchDocumentItemDto, SearchDocumentsParams, SearchDocumentsResult,
};
use super::{StrategyServer, internal_error, invalid_params};

impl StrategyServer {
    pub(crate) async fn search_documents_inner(
        &self,
        params: SearchDocumentsParams,
    ) -> Result<SearchDocumentsResult, McpError> {
        let rows = self
            .dependencies
            .document_search
            .search(DocumentSearchCriteria {
                query: params.query,
                as_of: params.as_of,
                stock_id: params.stock_id,
                limit: params.limit.unwrap_or(20),
            })
            .await
            .map_err(map_search_error)?;
        let mentions = self
            .dependencies
            .document_mentions
            .find_for_chunks(rows.iter().map(|row| row.chunk.id).collect())
            .await
            .map_err(map_mentions_error)?;
        let mut mentions_by_chunk = BTreeMap::<Uuid, Vec<DocumentMentionDto>>::new();
        for mention in mentions {
            mentions_by_chunk
                .entry(mention.chunk_id)
                .or_default()
                .push(DocumentMentionDto {
                    stock_id: mention.stock_id,
                    start_offset: mention.start_offset,
                    end_offset: mention.end_offset,
                });
        }
        let items = rows
            .into_iter()
            .map(|row| {
                let chunk_id = row.chunk.id;
                SearchDocumentItemDto {
                    document: row.document.into(),
                    version: row.version.into(),
                    chunk: row.chunk.into(),
                    mentions: mentions_by_chunk.remove(&chunk_id).unwrap_or_default(),
                }
            })
            .collect();
        Ok(SearchDocumentsResult { items })
    }

    pub(crate) async fn get_document_chunk_inner(
        &self,
        params: GetDocumentChunkParams,
    ) -> Result<GetDocumentChunkResult, McpError> {
        let context = self
            .dependencies
            .document_storage
            .get_chunk_context(params.id, params.as_of)
            .await
            .map_err(map_storage_error)?
            .ok_or_else(|| McpError::resource_not_found("document chunk not found", None))?;
        let mentions = self
            .dependencies
            .document_mentions
            .find_for_chunks(vec![context.chunk.id])
            .await
            .map_err(map_mentions_error)?
            .into_iter()
            .map(|mention| DocumentMentionDto {
                stock_id: mention.stock_id,
                start_offset: mention.start_offset,
                end_offset: mention.end_offset,
            })
            .collect();
        Ok(context_into_result(context, mentions))
    }
}

impl From<Document> for DocumentDto {
    fn from(document: Document) -> Self {
        Self {
            id: document.id,
            source: document.metadata.source,
            source_document_id: document.metadata.source_document_id,
            stock_id: document.metadata.stock_id,
            source_issuer_code: document.metadata.source_issuer_code,
            title: document.metadata.title,
            published_at: document.metadata.published_at,
        }
    }
}

impl From<core_application::document_search::DocumentSearchVersion>
    for DocumentVersionReferenceDto
{
    fn from(version: core_application::document_search::DocumentSearchVersion) -> Self {
        Self {
            id: version.id,
            content_hash: version.content_hash,
            retrieved_at: version.retrieved_at,
        }
    }
}

impl From<core_application::document_storage::DocumentVersion> for DocumentVersionDto {
    fn from(version: core_application::document_storage::DocumentVersion) -> Self {
        Self {
            id: version.id,
            content_hash: version.content_hash,
            retrieved_at: version.retrieved_at,
            body: version.body,
        }
    }
}

impl From<DocumentChunk> for DocumentChunkDto {
    fn from(chunk: DocumentChunk) -> Self {
        Self {
            id: chunk.id,
            sequence: chunk.sequence,
            start_offset: chunk.start_offset,
            end_offset: chunk.end_offset,
            body: chunk.body,
        }
    }
}

fn context_into_result(
    context: DocumentChunkContext,
    mentions: Vec<DocumentMentionDto>,
) -> GetDocumentChunkResult {
    GetDocumentChunkResult {
        document: context.document.into(),
        version: context.version.into(),
        previous_chunk: context.previous_chunk.map(Into::into),
        chunk: context.chunk.into(),
        next_chunk: context.next_chunk.map(Into::into),
        mentions,
    }
}

fn map_search_error(
    error: core_application::document_search::DocumentSearchUseCaseError,
) -> McpError {
    match error {
        core_application::document_search::DocumentSearchUseCaseError::InvalidQuery => {
            invalid_params(
                "query must contain at least one word, with every word at least 2 characters long",
            )
        }
        core_application::document_search::DocumentSearchUseCaseError::InvalidLimit => {
            invalid_params("limit must be between 1 and 100")
        }
        error => {
            tracing::error!(error = %error, "strategy mcp document search failed");
            internal_error(format!("database error: {error}"))
        }
    }
}

fn map_storage_error(
    error: core_application::document_storage::DocumentStorageUseCaseError,
) -> McpError {
    tracing::error!(error = %error, "strategy mcp document read failed");
    internal_error(format!("database error: {error}"))
}

fn map_mentions_error(
    error: core_application::document_mentions::DocumentMentionUseCasesError,
) -> McpError {
    tracing::error!(error = %error, "strategy mcp document mentions lookup failed");
    internal_error(format!("database error: {error}"))
}
