use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{ListMoversParams, ListMoversResult};

#[tool_router(router = movers_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 期間内の騰落率上位銘柄と戦略との接点を返す。
    #[tool(
        name = "list_movers",
        description = "Rank all instruments by daily close return over an inclusive date range and report the strategy's earliest contact in that range. from is compared with the previous available daily close; to uses the latest available close on or before that date, so from=to returns the daily change. direction=up returns gainers, down returns decliners, and abs ranks by absolute return. min_avg_turnover filters by average daily close × volume in each instrument's native currency; it defaults to 0. avg_turnover is also in the instrument's native currency. limit defaults to 50 and must be between 1 and 100. Unseen instruments have null first_seen_at and seen_via. seen_via is query_data, note, trade, paper_trade, or prediction.",
        annotations(read_only_hint = true)
    )]
    async fn list_movers(
        &self,
        Parameters(params): Parameters<ListMoversParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListMoversResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.list_movers_inner(scope, params).await.map(Json)
    }
}
