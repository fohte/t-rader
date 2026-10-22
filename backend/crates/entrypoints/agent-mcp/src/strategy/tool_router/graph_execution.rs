use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::dto::{
    CaptureDatasetParams, CaptureDatasetResult, ReadTaskExecutionSnapshotResult,
};
use super::super::{
    StrategyServer, execution_step_id_from_ctx, execution_task_id_from_ctx, invalid_params,
};

#[tool_router(router = graph_execution_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 登録済み provider から固定済み dataset payload を捕捉する。
    #[tool(
        name = "capture_dataset",
        description = "Capture a stable dataset from a registered provider and return its sealed task-scoped payload reference. Read values in bounded pages with read_execution_payload."
    )]
    async fn capture_dataset(
        &self,
        Parameters(params): Parameters<CaptureDatasetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<CaptureDatasetResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let a2a_task_id = execution_task_id_from_ctx(&ctx)
            .ok_or_else(|| invalid_params("x-execution-id must include an active task ID"))?;
        let execution_step_id = execution_step_id_from_ctx(&ctx).ok_or_else(|| {
            invalid_params("x-execution-id must include a valid execution step ID")
        })?;
        self.capture_dataset_inner(scope, &a2a_task_id, execution_step_id, params)
            .await
            .map(Json)
    }

    /// 同じ task の開始時に固定した graph と agent 定義を返す。
    #[tool(
        name = "read_task_execution_snapshot",
        description = "Read the immutable agent configuration and subagent definitions captured when the current task started. Returns a null definition for tasks created before snapshots were available.",
        annotations(read_only_hint = true)
    )]
    async fn read_task_execution_snapshot(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadTaskExecutionSnapshotResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let a2a_task_id = execution_task_id_from_ctx(&ctx)
            .ok_or_else(|| invalid_params("x-execution-id must include an active task ID"))?;
        self.read_task_execution_snapshot_inner(scope, &a2a_task_id)
            .await
            .map(Json)
    }
}
