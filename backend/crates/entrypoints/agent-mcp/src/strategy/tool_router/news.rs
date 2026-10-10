use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{
    GetNewsContentParams, GetNewsContentResult, SearchNewsParams, SearchNewsResult,
};

#[tool_router(router = news_tool_router, vis = "pub(super)")]
impl StrategyServer {
    #[tool(
        name = "get_news_content",
        description = "Read a news item's stored article body and content retrieval state by id. Use this after search_news when content_status is fetched. If content_status is anything other than fetched, use search_web with the item's title and/or URL; the returned search result includes the article body when available.",
        annotations(read_only_hint = true)
    )]
    async fn get_news_content(
        &self,
        Parameters(params): Parameters<GetNewsContentParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<GetNewsContentResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.get_news_content_inner(scope, params).await.map(Json)
    }

    /// news_item を title/body_snippet/保存済み本文のキーワードと published_at の期間で直接検索する
    #[tool(
        name = "search_news",
        description = "Search news_item by case-insensitive substring match against title, body_snippet, or stored article body, and/or by published_at date range, newest first. Returns content_status (null when no content record exists). Use get_news_content for items with content_status=fetched; otherwise search_web can search the item's title and/or URL and return its body when available.",
        annotations(read_only_hint = true)
    )]
    async fn search_news(
        &self,
        Parameters(params): Parameters<SearchNewsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<SearchNewsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.search_news_inner(scope, params).await.map(Json)
    }
}
