use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{
    PlacePaperOrderParams, PlacePaperOrderResult, ReadPaperPortfolioParams,
    ReadPaperPortfolioResult, ReadPaperStatsResult,
};

#[tool_router(router = paper_trade_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 戦略 task の purpose に対応する口座へ注文を記録する
    #[tool(
        name = "place_paper_order",
        description = "Record a paper market order for a Japanese stock. The account is selected from the current x-strategy-id and x-execution-id task purpose; do not choose an account. side must be buy or sell, qty must be a positive multiple of 100, and note_version_id must identify the note version that supports the decision. Orders cannot be edited or cancelled. The daily fill job fills at the opening price of the first daily bar after the order date and rejects orders with insufficient cash/shares or if bars remain unavailable for five trading sessions."
    )]
    async fn place_paper_order(
        &self,
        Parameters(params): Parameters<PlacePaperOrderParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<PlacePaperOrderResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let task_id = super::super::execution_task_id_from_ctx(&ctx);
        self.place_paper_order_inner(scope, task_id, params)
            .await
            .map(Json)
    }

    /// 口座 ID が指定されていればその口座を、なければ接続元 task の口座を返す
    #[tool(
        name = "read_paper_portfolio",
        description = "Read a specified paper account, or select the account for the current x-strategy-id and x-execution-id task purpose when account_id is omitted. Use an account_id returned by read_paper_stats to inspect any account; x-execution-id is required only when account_id is omitted. Returns cash, open positions with latest daily-close valuation, the order history and fill/rejection results, and each order's note_version_id. read_note requires the parent note_id and this value as version_id to inspect the decision version. A strategy scope is required.",
        annotations(read_only_hint = true)
    )]
    async fn read_paper_portfolio(
        &self,
        Parameters(params): Parameters<ReadPaperPortfolioParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadPaperPortfolioResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let task_id = super::super::execution_task_id_from_ctx(&ctx);
        self.read_paper_portfolio_inner(scope, task_id, params)
            .await
            .map(Json)
    }

    /// 全ペーパートレード口座の成績を返す
    #[tool(
        name = "read_paper_stats",
        description = "Compare every paper account. Returns total assets, return since account start and the same-period benchmark return, closed FIFO trade count, benchmark-relative win rate and average winning/losing excess returns, plus unrealized P&L. Returns are decimal ratios (0.05 means 5%). Benchmark return is null if the start or end price is unavailable. Trades without benchmark prices on both fill dates are omitted from trade metrics; win rate is null when no trade can be evaluated, and average returns are null when that outcome group is empty.",
        annotations(read_only_hint = true)
    )]
    async fn read_paper_stats(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadPaperStatsResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.read_paper_stats_inner().await.map(Json)
    }
}
