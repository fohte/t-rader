//! `eval_indicator` tool の inner method 実装。
//!
//! 戦略 scope (優先) → global scope の順に同名 indicator を解決し、`input_schema` で
//! 引数を validation した上で、Kata 上の exec Pod に code と `{"args": <args>}` を渡して
//! 実行する。stdout 最終行を JSON parse し、`output_schema` で validation した結果を返す。
//!
//! sandbox による拒否 (network / subprocess / fs write) は `eval_python` と同じく
//! `exit_code != 0` + `stderr` で透過する。MCP エラーには変換しない。

use std::time::Duration;

use core_application::custom_indicator::CustomIndicator;
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use core_application::kata_exec::{ExecRequest, KataExecError};

use super::dto::{EvalIndicatorParams, EvalIndicatorResult};
use super::{
    EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_STDIN_BYTES, EXEC_MAX_TIMEOUT_SECS, StrategyServer,
    check_exec_upper_bound, internal_error, invalid_params, kata_exec_to_mcp_err,
};

/// JSON Schema validation の失敗種別。stored schema 自体の不正と instance の不一致を
/// 呼び出し側で別の MCP error に振り分けるために区別する。
enum SchemaCheckError {
    /// schema 自体が JSON Schema として不正 (operator 側の保存ミス)。
    BrokenSchema(String),
    /// instance が schema に合致しない (caller 側の入力ミス、もしくは indicator 出力の問題)。
    Mismatch(String),
}

impl StrategyServer {
    pub(crate) async fn eval_indicator_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: EvalIndicatorParams,
    ) -> Result<EvalIndicatorResult, McpError> {
        let strategy_scope = scope.into();
        let session_strategy_id = strategy_scope.id();
        let name = params.name.trim();
        if name.is_empty() {
            return Err(invalid_params("name must not be empty"));
        }
        check_exec_upper_bound("timeout_secs", params.timeout_secs, EXEC_MAX_TIMEOUT_SECS)?;
        check_exec_upper_bound(
            "max_output_bytes",
            params.max_output_bytes,
            EXEC_MAX_OUTPUT_BYTES,
        )?;

        let indicator = self
            .dependencies
            .custom_indicators
            .resolve(strategy_scope, name)
            .await
            .map_err(|e| internal_error(format!("failed to resolve indicator: {e}")))?
            .ok_or_else(|| {
                McpError::resource_not_found(format!("indicator '{name}' not found"), None)
            })?;

        validate_with_schema(&indicator.input_schema, &params.args).map_err(|err| match err {
            SchemaCheckError::Mismatch(msg) => invalid_params(format!(
                "args do not match input_schema of indicator '{name}': {msg}"
            )),
            SchemaCheckError::BrokenSchema(msg) => internal_error(format!(
                "indicator '{name}' has invalid input_schema in storage: {msg}"
            )),
        })?;

        let executor = self.kata_executor()?;

        let stdin = serde_json::to_string(&serde_json::json!({ "args": params.args }))
            .map_err(|e| internal_error(format!("failed to serialize args: {e}")))?;
        if stdin.len() > EXEC_MAX_STDIN_BYTES {
            return Err(invalid_params(format!(
                "args exceed {EXEC_MAX_STDIN_BYTES} bytes when serialized as JSON"
            )));
        }

        let request = ExecRequest {
            code: indicator.code.clone(),
            stdin: Some(stdin),
            timeout: params.timeout_secs.map(|s| Duration::from_secs(s.into())),
            max_output_bytes: params.max_output_bytes.map(|m| m as usize),
        };

        tracing::info!(
            strategy_id = %session_strategy_id,
            indicator_id = %indicator.indicator_id,
            indicator_name = %indicator.name,
            indicator_scope = %indicator.scope,
            "eval_indicator: dispatching exec request",
        );

        let result = executor
            .run(request)
            .await
            .map_err(|e| kata_exec_error(e, session_strategy_id, &indicator))?;

        let output = if result.exit_code == 0 {
            Some(parse_and_validate_output(&result.stdout, &indicator)?)
        } else {
            None
        };

        Ok(EvalIndicatorResult {
            indicator_id: indicator.indicator_id,
            scope: indicator.scope,
            output,
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
        })
    }
}

fn validate_with_schema(schema: &JsonValue, instance: &JsonValue) -> Result<(), SchemaCheckError> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| SchemaCheckError::BrokenSchema(e.to_string()))?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(SchemaCheckError::Mismatch(errors.join("; ")))
    }
}

fn parse_and_validate_output(
    stdout: &str,
    indicator: &CustomIndicator,
) -> Result<JsonValue, McpError> {
    let last_line = stdout.lines().rfind(|l| !l.trim().is_empty());
    let Some(raw) = last_line else {
        return Err(invalid_params(format!(
            "indicator '{}' produced empty stdout; expected a JSON value on the last line",
            indicator.name
        )));
    };
    let parsed: JsonValue = serde_json::from_str(raw.trim()).map_err(|e| {
        invalid_params(format!(
            "indicator '{}' last stdout line is not valid JSON: {e}",
            indicator.name
        ))
    })?;
    validate_with_schema(&indicator.output_schema, &parsed).map_err(|err| match err {
        SchemaCheckError::Mismatch(msg) => invalid_params(format!(
            "output does not match output_schema of indicator '{}': {msg}",
            indicator.name
        )),
        SchemaCheckError::BrokenSchema(msg) => internal_error(format!(
            "indicator '{}' has invalid output_schema in storage: {msg}",
            indicator.name
        )),
    })?;
    Ok(parsed)
}

fn kata_exec_error(err: KataExecError, strategy_id: Uuid, indicator: &CustomIndicator) -> McpError {
    tracing::warn!(
        strategy_id = %strategy_id,
        indicator_id = %indicator.indicator_id,
        error = %err,
        "eval_indicator: kata exec error",
    );
    kata_exec_to_mcp_err(err)
}
