use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router};

use super::super::dto::{
    ExecuteOperationParams, ExecuteOperationResult, ReadExecutionPayloadParams,
    ReadExecutionPayloadResult,
};
use super::super::{
    StrategyServer, execution_step_id_from_ctx, execution_task_id_from_ctx, invalid_params,
};

#[tool_router(router = execution_payload_tool_router, vis = "pub(super)")]
impl StrategyServer {
    /// 保存済み定義を task に紐付けて実行し、入力・出力 payload の参照を返す
    #[tool(
        name = "execute_operation",
        description = "Execute a saved custom indicator within the current task. The definition and complete input/output are saved as task-scoped payloads; read them in bounded pages with read_execution_payload. Use payload_inputs to pass authorized sealed payload selections as bounded files. Set numeric_pointers only when numeric values should be available for note citations. Pointers must select numbers under the output payload's value field; omitting them still returns the payload reference."
    )]
    async fn execute_operation(
        &self,
        Parameters(params): Parameters<ExecuteOperationParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ExecuteOperationResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let a2a_task_id = execution_task_id_from_ctx(&ctx)
            .ok_or_else(|| invalid_params("x-execution-id must include an active task ID"))?;
        let execution_step_id = execution_step_id_from_ctx(&ctx).ok_or_else(|| {
            invalid_params("x-execution-id must include a valid execution step ID")
        })?;
        self.execute_operation_inner(scope, &a2a_task_id, execution_step_id, params)
            .await
            .map(Json)
    }

    /// task 内で seal 済み payload の有限ページを読む
    #[tool(
        name = "read_execution_payload",
        description = "Read a sealed payload belonging to the current task. JSON Pointer selects a value and cursor/limit return a finite page. Staging and incomplete payloads cannot be read.",
        annotations(read_only_hint = true)
    )]
    async fn read_execution_payload(
        &self,
        Parameters(params): Parameters<ReadExecutionPayloadParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadExecutionPayloadResult>, McpError> {
        let scope = self.strategy_scope_from_ctx(&ctx).await?;
        let a2a_task_id = execution_task_id_from_ctx(&ctx)
            .ok_or_else(|| invalid_params("x-execution-id must include an active task ID"))?;
        let execution_step_id = execution_step_id_from_ctx(&ctx);
        self.read_execution_payload_inner(scope, &a2a_task_id, execution_step_id, params)
            .await
            .map(Json)
    }
}
