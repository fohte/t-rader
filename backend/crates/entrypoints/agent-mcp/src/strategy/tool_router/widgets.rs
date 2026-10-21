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
        description = "Evaluate a widget definition. Supported metrics are change, close, turnover, mkt_cap, per, pbr, and indicator; indicator requires by=none, an indicator ID, and no aggregation. Treemap, toplist, and table return cross-sectional rows. Line requires time.range set to 3m, 1y, or an inclusive YYYY-MM-DD..YYYY-MM-DD range, and returns dated series points. Pass the widget YAML without a fenced block. Relative ranges use time.as_of when set, otherwise the latest available date is resolved for the requested data. Note widget blocks require time.as_of.",
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
