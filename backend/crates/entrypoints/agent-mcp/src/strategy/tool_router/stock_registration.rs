use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::stock_registration::{RegisterStockParams, RegisterStockResult};

#[tool_router(router = stock_registration_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 外国株を銘柄マスタに登録する
    #[tool(
        name = "register_stock",
        description = "Register a foreign stock using an ISO 3166-1 alpha-2 country code, stock code, name, and exchange name. Japanese stocks are registered by synchronization and cannot be registered here."
    )]
    async fn register_stock(
        &self,
        Parameters(params): Parameters<RegisterStockParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<RegisterStockResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.register_stock_inner(params).await.map(Json)
    }
}
