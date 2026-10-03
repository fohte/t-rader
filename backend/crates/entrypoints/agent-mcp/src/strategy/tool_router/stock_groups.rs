use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::stock_groups::{
    CreateStockGroupParams, ListStockGroupMembersParams, StockGroupDto,
    StockGroupMemberChangeResult, StockGroupMemberParams, UpdateStockGroupParams,
};
use super::super::stock_registration::{RegisterStockParams, RegisterStockResult};

#[tool_router(router = stock_groups_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 外国株を銘柄マスタに登録する
    #[tool(
        name = "register_stock",
        description = "Register a foreign stock using an ISO 3166-1 alpha-2 country code and exchange code. Japanese stocks are registered by synchronization and cannot be registered here."
    )]
    async fn register_stock(
        &self,
        Parameters(params): Parameters<RegisterStockParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<RegisterStockResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.register_stock_inner(params).await.map(Json)
    }

    /// 分類軸のグループを作成する
    #[tool(
        name = "create_stock_group",
        description = "Create a stock group under an existing group axis. The axis must be managed by an agent rather than a synchronization source. Group keys are immutable."
    )]
    async fn create_stock_group(
        &self,
        Parameters(params): Parameters<CreateStockGroupParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<StockGroupDto>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.create_stock_group_inner(params).await.map(Json)
    }

    /// グループの表示名と説明を更新する
    #[tool(
        name = "update_stock_group",
        description = "Update a stock group's name and/or description. Its axis key and group key are immutable; pass description as null to clear it. Groups on synchronized axes cannot be changed by MCP."
    )]
    async fn update_stock_group(
        &self,
        Parameters(params): Parameters<UpdateStockGroupParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<StockGroupDto>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.update_stock_group_inner(params).await.map(Json)
    }

    /// グループに銘柄を追加する
    #[tool(
        name = "add_stock_to_group",
        description = "Add one stock to a group. Repeating an existing membership is a no-op. The stock ID must exist, and groups on synchronized axes cannot be changed by MCP."
    )]
    async fn add_stock_to_group(
        &self,
        Parameters(params): Parameters<StockGroupMemberParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<StockGroupMemberChangeResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.add_stock_to_group_inner(params).await.map(Json)
    }

    /// グループから銘柄を削除する
    #[tool(
        name = "remove_stock_from_group",
        description = "Remove one stock from a group. Repeating a removal for a non-member is a no-op. Groups on synchronized axes cannot be changed by MCP."
    )]
    async fn remove_stock_from_group(
        &self,
        Parameters(params): Parameters<StockGroupMemberParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<StockGroupMemberChangeResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.remove_stock_from_group_inner(params).await.map(Json)
    }

    /// グループに属する銘柄一覧を返す
    #[tool(
        name = "list_stock_group_members",
        description = "List the stock IDs in a group in ascending order. Stock groups are account-wide and do not belong to the calling strategy.",
        annotations(read_only_hint = true)
    )]
    async fn list_stock_group_members(
        &self,
        Parameters(params): Parameters<ListStockGroupMembersParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<super::super::stock_groups::ListStockGroupMembersResult>, McpError> {
        self.strategy_scope_from_ctx(&ctx).await?;
        self.list_stock_group_members_inner(params).await.map(Json)
    }
}
