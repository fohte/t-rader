use std::fmt::Write as _;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResponse, CallToolResult};
use rmcp::service::{RequestContext, RoleServer};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    StrategyServer, execution_id_from_ctx, execution_step_id_from_ctx,
    execution_task_id_for_read_from_ctx, execution_task_id_from_ctx, internal_error,
    invalid_params, strategy_task_id_from_ctx,
};

impl StrategyServer {
    pub(super) async fn call_tool_with_execution_budget(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if request.name == "__execution_budget" {
            return self
                .call_execution_budget_operation(request.arguments, &context)
                .await;
        }

        let allow_task_only_read = matches!(
            request.name.as_ref(),
            "read_task_execution_snapshot" | "read_execution_payload"
        );
        let is_step_scoped = execution_step_id_from_ctx(&context).is_some();
        let reserved_call = if let Some(access) = self
            .authorize_budget_execution(&context, allow_task_only_read)
            .await?
        {
            // Task-only readers use independent MCP clients whose request IDs can repeat.
            if allow_task_only_read && !is_step_scoped {
                None
            } else {
                let execution_id = execution_id_from_ctx(&context).ok_or_else(|| {
                    invalid_params("x-execution-id is required for budgeted tool calls")
                })?;
                let call_id = format!("{execution_id}:{}", context.id);
                let subagent = subagent_identity_from_ctx(&context)?;
                let fingerprint = request_fingerprint(&serde_json::json!({
                    "name": request.name.as_ref(),
                    "arguments": request.arguments.clone(),
                }))?;
                let default_limit = match request.name.as_ref() {
                    super::read_page::TOOL_NAME => {
                        Some(u64::from(super::read_page::READ_PAGE_MAX_CALLS_PER_TASK))
                    }
                    super::web_search::TOOL_NAME => {
                        Some(u64::from(super::web_search::SEARCH_WEB_MAX_CALLS_PER_TASK))
                    }
                    _ => None,
                };
                let reservation = self
                    .dependencies
                    .execution_budgets
                    .reserve_tool_call(
                        &access,
                        call_id.clone(),
                        fingerprint,
                        request.name.as_ref(),
                        default_limit,
                        subagent.as_ref().map(|(id, key)| {
                            core_application::execution_budget::SubagentExecutionIdentity {
                                execution_id: id,
                                definition_key: key,
                            }
                        }),
                    )
                    .await
                    .map_err(execution_budget_error_to_mcp)?;
                Some((access, call_id, reservation))
            }
        } else {
            None
        };

        match reserved_call {
            Some((access, call_id, reservation)) => {
                self.dispatch_reserved_tool(access, call_id, request, context, reservation)
                    .await
            }
            None => {
                Self::tool_router()
                    .call(ToolCallContext::new(self, request, context))
                    .await
            }
        }
    }

    async fn dispatch_reserved_tool(
        &self,
        access: core_application::strategy_execution::AuthorizedExecution,
        call_id: String,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
        reservation: core_application::execution_budget::BudgetReservationResult,
    ) -> Result<CallToolResponse, McpError> {
        if let Some(response) = reservation.completed_response {
            return serde_json::from_value(response)
                .map(CallToolResponse::Complete)
                .map_err(|error| {
                    internal_error(format!("cached strategy tool response is invalid: {error}"))
                });
        }
        if reservation.reservations.is_empty()
            || reservation.reservations.iter().any(|reservation| {
                reservation.state
                    != core_application::execution_budget::BudgetReservationState::Reserved
            })
        {
            return Err(invalid_params(
                "tool call is already dispatched and its result is unavailable",
            ));
        }
        let transition = self
            .dependencies
            .execution_budgets
            .transition_call(
                &access,
                &call_id,
                core_application::execution_budget::BudgetReservationState::Consumed,
            )
            .await
            .map_err(execution_budget_error_to_mcp)?;
        if !transition.transitioned {
            return Err(invalid_params("tool call dispatch is already claimed"));
        }

        let response = Self::tool_router()
            .call(ToolCallContext::new(self, request, context))
            .await?;
        if let CallToolResponse::Complete(result) = &response {
            match serde_json::to_value(result) {
                Ok(serialized) => {
                    if let Err(error) = self
                        .dependencies
                        .execution_budgets
                        .complete_call(&access, &call_id, serialized)
                        .await
                    {
                        tracing::error!(
                            error = %error,
                            call_id,
                            "failed to persist strategy mcp tool response",
                        );
                    }
                }
                Err(error) => tracing::error!(
                    error = %error,
                    "failed to serialize strategy mcp tool response",
                ),
            }
        }
        Ok(response)
    }

    async fn authorize_budget_execution(
        &self,
        context: &RequestContext<RoleServer>,
        allow_task_only_read: bool,
    ) -> Result<Option<core_application::strategy_execution::AuthorizedExecution>, McpError> {
        let parts = context
            .extensions
            .get::<axum::http::request::Parts>()
            .ok_or_else(|| internal_error("missing http parts in mcp request context"))?;
        if optional_header(&parts.headers, "x-execution-id")?.is_none() {
            if strategy_task_id_from_ctx(context)?.is_some() {
                return Err(invalid_params(
                    "x-execution-id is required for budgeted requests",
                ));
            }
            return Ok(None);
        }
        let a2a_task_id = if allow_task_only_read {
            execution_task_id_for_read_from_ctx(context)
        } else {
            execution_task_id_from_ctx(context)
        }
        .ok_or_else(|| invalid_params("x-execution-id must include an active task ID"))?;
        let task_id = strategy_task_id_from_ctx(context)?
            .ok_or_else(|| invalid_params("missing x-strategy-task-id header"))?;
        let execution_step_id = execution_step_id_from_ctx(context);
        if !allow_task_only_read && execution_step_id.is_none() {
            return Err(invalid_params(
                "x-execution-id must include a valid execution step ID",
            ));
        }
        let scope = self.strategy_scope_from_ctx(context).await?;
        let access = self
            .dependencies
            .strategy_executions
            .authorize_execution(scope, &a2a_task_id, execution_step_id)
            .await
            .map_err(strategy_execution_error_to_mcp)?;
        if access.task_id() != task_id {
            return Err(invalid_params(
                "x-strategy-task-id does not match the active task",
            ));
        }
        Ok(Some(access))
    }

    async fn call_execution_budget_operation(
        &self,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let access = self
            .authorize_budget_execution(context, false)
            .await?
            .ok_or_else(|| invalid_params("budget operation requires an active task"))?;
        let arguments = serde_json::Value::Object(arguments.unwrap_or_default());
        let fingerprint = request_fingerprint(&arguments)?;
        let action = required_string(&arguments, "action")?;
        let call_id = required_string(&arguments, "call_id")?;
        match action {
            "reserve_model_call" => {
                let execution_step_id = execution_step_id_from_ctx(context).ok_or_else(|| {
                    invalid_params("x-execution-id must include a valid execution step ID")
                })?;
                let subagent = subagent_identity_from_ctx(context)?;
                let reservation = self
                    .dependencies
                    .execution_budgets
                    .reserve_model_call(
                        &access,
                        call_id.to_string(),
                        fingerprint,
                        &execution_step_id.to_string(),
                        subagent.as_ref().map(|(id, key)| {
                            core_application::execution_budget::SubagentExecutionIdentity {
                                execution_id: id,
                                definition_key: key,
                            }
                        }),
                    )
                    .await
                    .map_err(execution_budget_error_to_mcp)?;
                Ok(reservation_availability_response(reservation))
            }
            "reserve_subagent_run" => {
                let parent_step_id = required_string(&arguments, "parent_execution_step_id")?;
                Uuid::parse_str(parent_step_id)
                    .map_err(|_| invalid_params("parent execution step ID is invalid"))?;
                let reservation = self
                    .dependencies
                    .execution_budgets
                    .reserve_subagent_run(
                        &access,
                        call_id.to_string(),
                        fingerprint,
                        required_string(&arguments, "phase_key")?,
                        parent_step_id,
                        required_string(&arguments, "definition_key")?,
                    )
                    .await
                    .map_err(execution_budget_error_to_mcp)?;
                Ok(reservation_availability_response(reservation))
            }
            "transition" => {
                let state = match required_string(&arguments, "state")? {
                    "consumed" => {
                        core_application::execution_budget::BudgetReservationState::Consumed
                    }
                    "released" => {
                        core_application::execution_budget::BudgetReservationState::Released
                    }
                    _ => return Err(invalid_params("reservation state is invalid")),
                };
                let transition = self
                    .dependencies
                    .execution_budgets
                    .transition_call(&access, call_id, state)
                    .await
                    .map_err(execution_budget_error_to_mcp)?;
                Ok(CallToolResponse::Complete(CallToolResult::structured(
                    serde_json::json!({"transitioned": transition.transitioned}),
                )))
            }
            _ => Err(invalid_params("budget operation is invalid")),
        }
    }
}

fn reservation_availability_response(
    reservation: core_application::execution_budget::BudgetReservationResult,
) -> CallToolResponse {
    CallToolResponse::Complete(CallToolResult::structured(serde_json::json!({
        "available": reservation.completed_response.is_none()
            && !reservation.reservations.is_empty()
            && reservation.reservations.iter().all(|reservation| {
                reservation.state
                    == core_application::execution_budget::BudgetReservationState::Reserved
            }),
    })))
}

fn request_fingerprint(value: &serde_json::Value) -> Result<String, McpError> {
    let serialized = serde_json::to_vec(value)
        .map_err(|error| internal_error(format!("failed to encode budgeted request: {error}")))?;
    let digest = Sha256::digest(serialized);
    let mut fingerprint = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(fingerprint, "{byte:02x}");
    }
    Ok(fingerprint)
}

fn required_string<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, McpError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_params(format!("{key} is required")))
}

fn subagent_identity_from_ctx(
    context: &RequestContext<RoleServer>,
) -> Result<Option<(String, String)>, McpError> {
    let parts = context
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| internal_error("missing http parts in mcp request context"))?;
    let execution_id = optional_header(&parts.headers, "x-subagent-execution-id")?;
    let definition_key = optional_header(&parts.headers, "x-subagent-definition-key")?;
    match (execution_id, definition_key) {
        (None, None) => Ok(None),
        (Some(execution_id), Some(definition_key)) => Ok(Some((execution_id, definition_key))),
        _ => Err(invalid_params(
            "subagent execution headers must be provided together",
        )),
    }
}

fn optional_header(
    headers: &axum::http::HeaderMap,
    name: &'static str,
) -> Result<Option<String>, McpError> {
    let Some(header) = headers.get(name) else {
        return Ok(None);
    };
    let value = header
        .to_str()
        .map_err(|_| invalid_params(format!("{name} header is not valid ASCII")))?
        .trim();
    if value.is_empty() {
        return Err(invalid_params(format!("{name} header must not be empty")));
    }
    Ok(Some(value.to_string()))
}

fn execution_budget_error_to_mcp(
    error: core_application::execution_budget::ExecutionBudgetUseCaseError,
) -> McpError {
    match error {
        core_application::execution_budget::ExecutionBudgetUseCaseError::InvalidRequest
        | core_application::execution_budget::ExecutionBudgetUseCaseError::Repository(
            core_application::execution_budget::ExecutionBudgetRepositoryError::DeadlineExpired
            | core_application::execution_budget::ExecutionBudgetRepositoryError::LimitExceeded {
                ..
            }
            | core_application::execution_budget::ExecutionBudgetRepositoryError::ReservationConflict
            | core_application::execution_budget::ExecutionBudgetRepositoryError::InvalidReservationState
            | core_application::execution_budget::ExecutionBudgetRepositoryError::UsageNotMeasured,
        ) => invalid_params(error.to_string()),
        other => {
            tracing::error!(error = %other, "strategy mcp execution budget operation failed");
            internal_error(format!("execution budget operation failed: {other}"))
        }
    }
}

fn strategy_execution_error_to_mcp(
    error: core_application::strategy_execution::StrategyExecutionUseCaseError,
) -> McpError {
    match error {
        core_application::strategy_execution::StrategyExecutionUseCaseError::TaskNotFound
        | core_application::strategy_execution::StrategyExecutionUseCaseError::StrategyMismatch
        | core_application::strategy_execution::StrategyExecutionUseCaseError::TaskNotRunning
        | core_application::strategy_execution::StrategyExecutionUseCaseError::TaskDeadlineExpired => {
            invalid_params("execution does not match an active task")
        }
        other => {
            tracing::error!(error = %other, "strategy mcp task authorization failed");
            internal_error(format!("task authorization failed: {other}"))
        }
    }
}
