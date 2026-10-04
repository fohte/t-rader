use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{SearchNewsParams, SearchNewsResult};

#[tool_router(router = news_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// news_item を title/body_snippet のキーワードと published_at の期間で直接検索する
    #[tool(
        name = "search_news",
        description = "Search news_item directly by keyword (case-insensitive substring match against title or body_snippet) and/or a published_at date range, newest first. body_snippet is truncated to the first 280 characters of the source feed's description, not the full article; use search_web with the title if you need more than that.",
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
