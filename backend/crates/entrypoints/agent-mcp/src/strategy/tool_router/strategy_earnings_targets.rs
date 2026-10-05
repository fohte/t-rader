use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::strategy_earnings_targets::{
    EarningsTargetChangeResult, EarningsTargetParams, ListEarningsTargetsResult,
};

#[tool_router(router = strategy_earnings_targets_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 戦略が決算を追う銘柄またはグループを追加する
    #[tool(
        name = "add_earnings_target",
        description = "Add a stock or group to the current strategy's earnings targets. Set ref_kind to stock or group. Group ref_id values use axis_key/group_key. Repeating an existing target is a no-op."
    )]
    async fn add_earnings_target(
        &self,
        Parameters(params): Parameters<EarningsTargetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<EarningsTargetChangeResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.add_earnings_target_inner(scope, params)
            .await
            .map(Json)
    }

    /// 戦略が決算を追う銘柄またはグループを削除する
    #[tool(
        name = "remove_earnings_target",
        description = "Remove a stock or group from the current strategy's earnings targets. Set ref_kind to stock or group. Group ref_id values use axis_key/group_key. Repeating a removal is a no-op."
    )]
    async fn remove_earnings_target(
        &self,
        Parameters(params): Parameters<EarningsTargetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<EarningsTargetChangeResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.remove_earnings_target_inner(scope, params)
            .await
            .map(Json)
    }

    /// 現在の戦略が決算を追う銘柄とグループを一覧する
    #[tool(
        name = "list_earnings_targets",
        description = "List the stocks and groups tracked for earnings by the current strategy, sorted by reference kind and ID.",
        annotations(read_only_hint = true)
    )]
    async fn list_earnings_targets(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListEarningsTargetsResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        self.list_earnings_targets_inner(scope).await.map(Json)
    }
}
