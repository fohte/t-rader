use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::Json;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::{tool, tool_router};

use super::super::StrategyServer;
use super::super::dto::ListSavedScreensResult;

#[tool_router(router = saved_screens_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 保存したスクリーナー条件の名前と中身を返す。
    #[tool(
        name = "list_saved_screens",
        description = "List saved screener conditions by name, including each condition's filters and sort order. Saved screens can only be changed through the frontend API.",
        annotations(read_only_hint = true)
    )]
    async fn list_saved_screens(
        &self,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListSavedScreensResult>, McpError> {
        self.list_saved_screens_inner().await.map(Json)
    }
}
