use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{QueryWidgetParams, QueryWidgetResult};

#[tool_router(router = widget_tool_router, vis = "pub(super)")]
impl StrategyServer {
    #[tool(
        name = "query_widget",
        description = "Evaluate a widget definition and return its values as table rows. Pass the YAML content for a widget fenced block; omit the fence. The query resolves the latest available date when time.as_of is omitted; include time.as_of in the note block to keep its values fixed.",
        annotations(read_only_hint = true)
    )]
    async fn query_widget(
        &self,
        Parameters(params): Parameters<QueryWidgetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<QueryWidgetResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.query_widget_inner(params).await.map(Json)
    }
}
