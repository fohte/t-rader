use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{
    GetDocumentChunkParams, GetDocumentChunkResult, SearchDocumentsParams, SearchDocumentsResult,
};

#[tool_router(router = documents_tool_router, vis = "pub(super)")]
impl StrategyServer {
    #[tool(
        name = "search_documents",
        description = "Search stored document chunks by full-text query. Optionally restrict results to a publication and retrieval time or an issuing stock. Each result includes the document issuer, source version metadata, chunk body and offsets, and separately identified stock mentions.",
        annotations(read_only_hint = true)
    )]
    async fn search_documents(
        &self,
        Parameters(params): Parameters<SearchDocumentsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchDocumentsResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.search_documents_inner(params).await.map(Json)
    }

    #[tool(
        name = "get_document_chunk",
        description = "Read a stored document chunk by id with the nearest preceding and following chunks from the same source version, document metadata, the original version body, and stock mentions in the requested chunk. When as_of is supplied, chunks from a later publication or retrieval are not returned.",
        annotations(read_only_hint = true)
    )]
    async fn get_document_chunk(
        &self,
        Parameters(params): Parameters<GetDocumentChunkParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<GetDocumentChunkResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.get_document_chunk_inner(params).await.map(Json)
    }
}
