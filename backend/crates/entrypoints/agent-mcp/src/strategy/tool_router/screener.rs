use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::{ScreenStocksParams, ScreenStocksResult};

#[tool_router(router = screener_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 画面と同じ条件で全銘柄を絞り込む。
    #[tool(
        name = "screen_stocks",
        description = "Filter all stocks with the same criteria and ordering as the stock screener. markets matches exact stock.market values. sectors accepts keys from the TSE 33 industry groups. PER and PBR are ratios; ROE and dividend yield are percentages; market_cap is in 100 million yen. A stock with no value for a metric is excluded only when a range for that metric is supplied. sort_by accepts id, name, market, sector, close, per, pbr, roe, dividend_yield, or market_cap and defaults to market_cap; sort_order accepts asc or desc and defaults to desc. page is 1-based and defaults to 1; each page contains up to 100 stocks. total_count is the count across all matching pages, and as_of is the latest valuation date.",
        annotations(read_only_hint = true)
    )]
    async fn screen_stocks(
        &self,
        Parameters(params): Parameters<ScreenStocksParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ScreenStocksResult>, McpError> {
        self.screen_stocks_inner(params).await.map(Json)
    }
}
