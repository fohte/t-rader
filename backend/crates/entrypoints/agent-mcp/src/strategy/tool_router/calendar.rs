use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::calendar::{ReadCalendarParams, ReadCalendarResult};

#[tool_router(router = calendar_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 指定期間のイベントを一覧する
    #[tool(
        name = "read_calendar",
        description = "Read indicator, central-bank, and earnings events for an inclusive date range. Omit from and to to read the current JST week. Earnings for stocks and groups tracked by the current strategy are listed individually; other earnings are summarized by country and date.",
        annotations(read_only_hint = true)
    )]
    async fn read_calendar(
        &self,
        Parameters(params): Parameters<ReadCalendarParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadCalendarResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.read_calendar_inner(scope, params).await.map(Json)
    }
}
