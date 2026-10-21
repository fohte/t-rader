use std::time::Duration;

use core_application::{
    kata_exec::ExecRequest,
    strategy_execution::{
        MAX_PAYLOAD_PAGE_ITEMS, PayloadDefinition, PayloadKind, ReadPayloadQuery,
        StrategyExecutionUseCaseError,
    },
    strategy_scope::StrategyScope,
};
use rmcp::ErrorData as McpError;
use serde_json::json;
use uuid::Uuid;

use super::dto::{
    ExecuteOperationParams, ExecuteOperationResult, ExecutionPayloadRefDto,
    ReadExecutionPayloadParams, ReadExecutionPayloadResult,
};
use super::eval_indicator::{
    SchemaCheckError, kata_exec_error, parse_and_validate_output, validate_with_schema,
};
use super::{
    EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_STDIN_BYTES, EXEC_MAX_TIMEOUT_SECS, StrategyServer,
    check_exec_upper_bound, internal_error, invalid_params,
};

impl StrategyServer {
    pub(crate) async fn execute_operation_inner(
        &self,
        scope: StrategyScope,
        a2a_task_id: &str,
        execution_step_id: Uuid,
        params: ExecuteOperationParams,
    ) -> Result<ExecuteOperationResult, McpError> {
        let name = params.name.trim();
        if name.is_empty() {
            return Err(invalid_params("name must not be empty"));
        }
        if params.numeric_pointers.is_empty()
            || params.numeric_pointers.len() > MAX_PAYLOAD_PAGE_ITEMS
            || params
                .numeric_pointers
                .keys()
                .any(|key| key.trim().is_empty())
        {
            return Err(invalid_params(
                "numeric_pointers must contain bounded non-empty names",
            ));
        }
        check_exec_upper_bound("timeout_secs", params.timeout_secs, EXEC_MAX_TIMEOUT_SECS)?;
        check_exec_upper_bound(
            "max_output_bytes",
            params.max_output_bytes,
            EXEC_MAX_OUTPUT_BYTES,
        )?;

        let access = self
            .dependencies
            .strategy_executions
            .authorize_execution(scope, a2a_task_id, Some(execution_step_id))
            .await
            .map_err(strategy_execution_error)?;
        let resolved = self
            .dependencies
            .strategy_executions
            .resolve_custom_indicator(scope, &access, name)
            .await
            .map_err(strategy_execution_error)?;
        let indicator = resolved.definition;
        validate_with_schema(&indicator.input_schema, &params.args).map_err(
            |error| match error {
                SchemaCheckError::Mismatch(message) => invalid_params(format!(
                    "args do not match input_schema of definition '{name}': {message}"
                )),
                SchemaCheckError::BrokenSchema(message) => {
                    internal_error(format!("stored input_schema is invalid: {message}"))
                }
            },
        )?;

        let executor = self.kata_executor()?;
        let args = params.args.clone();
        let input = json!({ "args": args });
        let stdin = serde_json::to_string(&input)
            .map_err(|_| internal_error("failed to serialize operation input"))?;
        if stdin.len() > EXEC_MAX_STDIN_BYTES {
            return Err(invalid_params(format!(
                "args exceed {EXEC_MAX_STDIN_BYTES} bytes when serialized as JSON"
            )));
        }

        let input_schema = json!({
            "type": "object",
            "properties": {"args": indicator.input_schema.clone()},
            "required": ["args"],
            "additionalProperties": false,
        });
        let definition_key = format!("custom_indicator:{name}");
        let definition_digest = resolved.digest;
        let input_payload = self
            .dependencies
            .strategy_executions
            .write_payload(
                &access,
                PayloadDefinition {
                    kind: PayloadKind::Input,
                    source: format!("custom_indicator:{name}"),
                    source_revision: Some(definition_digest.clone()),
                    source_locator: Some(json!({ "definition_key": definition_key })),
                    source_metadata: json!({ "definition_digest": definition_digest }),
                    schema: input_schema,
                },
                input,
            )
            .await
            .map_err(strategy_execution_error)?;

        let max_output_bytes = params.max_output_bytes.unwrap_or(EXEC_MAX_OUTPUT_BYTES);
        let result = executor
            .run(ExecRequest {
                code: indicator.code.clone(),
                stdin: Some(stdin),
                timeout: params
                    .timeout_secs
                    .map(|seconds| Duration::from_secs(seconds.into())),
                max_output_bytes: Some(max_output_bytes as usize),
            })
            .await
            .map_err(|error| kata_exec_error(error, scope.id(), &indicator))?;

        let output = if result.exit_code == 0 {
            Some(parse_and_validate_output(&result.stdout, &indicator)?)
        } else {
            None
        };
        let output_value = json!({
            "exit_code": result.exit_code,
            "stdout": result.stdout.clone(),
            "stderr": result.stderr.clone(),
            "value": output.clone(),
        });
        let output_schema = json!({
            "type": "object",
            "properties": {
                "exit_code": {"type": "integer"},
                "stdout": {"type": "string"},
                "stderr": {"type": "string"},
                "value": {"anyOf": [indicator.output_schema.clone(), {"type": "null"}]},
            },
            "required": ["exit_code", "stdout", "stderr", "value"],
            "additionalProperties": false,
        });
        let output_payload = self
            .dependencies
            .strategy_executions
            .write_payload(
                &access,
                PayloadDefinition {
                    kind: PayloadKind::Output,
                    source: format!("custom_indicator:{name}"),
                    source_revision: Some(definition_digest.clone()),
                    source_locator: Some(json!({
                        "definition_key": format!("custom_indicator:{name}")
                    })),
                    source_metadata: json!({ "definition_digest": definition_digest }),
                    schema: output_schema,
                },
                output_value.clone(),
            )
            .await
            .map_err(strategy_execution_error)?;

        let evaluation_id = if output.is_some() {
            Some(
                self.dependencies
                    .strategy_executions
                    .record_numeric_projection(
                        &access,
                        output_payload.id,
                        &indicator,
                        args,
                        params.numeric_pointers,
                    )
                    .await
                    .map_err(strategy_execution_error)?,
            )
        } else {
            None
        };

        Ok(ExecuteOperationResult {
            input_payload: input_payload.into(),
            output_payload: Some(output_payload.into()),
            evaluation_id,
            exit_code: result.exit_code,
            stderr: result.stderr,
        })
    }

    pub(crate) async fn read_execution_payload_inner(
        &self,
        scope: StrategyScope,
        a2a_task_id: &str,
        execution_step_id: Option<Uuid>,
        params: ReadExecutionPayloadParams,
    ) -> Result<ReadExecutionPayloadResult, McpError> {
        let access = self
            .dependencies
            .strategy_executions
            .authorize_execution(scope, a2a_task_id, execution_step_id)
            .await
            .map_err(strategy_execution_error)?;
        let page = self
            .dependencies
            .strategy_executions
            .read_payload(
                &access,
                ReadPayloadQuery {
                    payload_id: params.payload_id,
                    pointer: params.pointer,
                    cursor: params.cursor,
                    limit: params.limit,
                },
            )
            .await
            .map_err(strategy_execution_error)?;
        Ok(ReadExecutionPayloadResult {
            payload_id: page.payload_id,
            pointer: page.pointer,
            data: page.data,
            cursor: page.cursor,
            next_cursor: page.next_cursor,
            total_count: page.total_count,
            digest: page.digest,
        })
    }
}

impl From<core_application::strategy_execution::PayloadRef> for ExecutionPayloadRefDto {
    fn from(payload: core_application::strategy_execution::PayloadRef) -> Self {
        Self {
            id: payload.id,
            task_id: payload.task_id,
            kind: payload.kind.as_str().to_string(),
            digest: payload.digest,
            item_count: payload.item_count,
            byte_count: payload.byte_count,
            source_revision: payload.source_revision,
            source_locator: payload.source_locator,
            source_metadata: payload.source_metadata,
        }
    }
}

fn strategy_execution_error(error: StrategyExecutionUseCaseError) -> McpError {
    match error {
        error @ (StrategyExecutionUseCaseError::TaskNotFound
        | StrategyExecutionUseCaseError::StrategyMismatch
        | StrategyExecutionUseCaseError::TaskNotRunning
        | StrategyExecutionUseCaseError::CustomIndicatorNotFound
        | StrategyExecutionUseCaseError::PayloadNotFound
        | StrategyExecutionUseCaseError::PayloadNotSealed
        | StrategyExecutionUseCaseError::InvalidPayloadPointer
        | StrategyExecutionUseCaseError::InvalidPageLimit
        | StrategyExecutionUseCaseError::PayloadPageTooLarge
        | StrategyExecutionUseCaseError::InvalidNumericProjection
        | StrategyExecutionUseCaseError::InvalidDatasetCursor) => invalid_params(error.to_string()),
        error => {
            tracing::error!(error = %error, "strategy execution payload operation failed");
            internal_error("strategy execution payload operation failed")
        }
    }
}
