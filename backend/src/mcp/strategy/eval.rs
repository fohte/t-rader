//! `eval_python` tool の inner method 実装。
//!
//! Python コードを Kata Containers 上の exec Pod で 1 回実行し、stdout/stderr/exit_code
//! を返す。MCP 層では timeout / 出力サイズ / code / stdin のバイト数上限を Pod 起動前に
//! 検査し、超過時は invalid_params を返す。

use std::time::Duration;

use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;
use uuid::Uuid;

use core_application::kata_exec::{ExecRequest, KataExecError};

use super::dto::{EvalPythonParams, EvalPythonResult};
use super::{
    EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_STDIN_BYTES, EXEC_MAX_TIMEOUT_SECS, StrategyServer,
    check_exec_upper_bound, invalid_params, kata_exec_to_mcp_err,
};

/// MCP 層で許容する Python コード本体のサイズ上限 (バイト)。
pub(super) const MAX_CODE_BYTES: usize = 64 * 1024;

impl StrategyServer {
    pub(crate) async fn eval_python_inner(
        &self,
        scope: impl Into<StrategyScope>,
        params: EvalPythonParams,
    ) -> Result<EvalPythonResult, McpError> {
        let session_strategy_id = scope.into().id();
        if params.code.is_empty() {
            return Err(invalid_params("code must not be empty"));
        }
        if params.code.len() > MAX_CODE_BYTES {
            return Err(invalid_params(format!(
                "code exceeds {MAX_CODE_BYTES} bytes"
            )));
        }
        if let Some(stdin) = params.stdin.as_ref()
            && stdin.len() > EXEC_MAX_STDIN_BYTES
        {
            return Err(invalid_params(format!(
                "stdin exceeds {EXEC_MAX_STDIN_BYTES} bytes"
            )));
        }
        check_exec_upper_bound("timeout_secs", params.timeout_secs, EXEC_MAX_TIMEOUT_SECS)?;
        check_exec_upper_bound(
            "max_output_bytes",
            params.max_output_bytes,
            EXEC_MAX_OUTPUT_BYTES,
        )?;

        let executor = self.kata_executor()?;

        let request = ExecRequest {
            code: params.code,
            stdin: params.stdin,
            timeout: params.timeout_secs.map(|s| Duration::from_secs(s.into())),
            max_output_bytes: params.max_output_bytes.map(|m| m as usize),
        };

        tracing::info!(
            strategy_id = %session_strategy_id,
            "eval_python: dispatching exec request",
        );

        let result = executor
            .run(request)
            .await
            .map_err(|e| kata_exec_error(e, session_strategy_id))?;

        Ok(EvalPythonResult {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
        })
    }
}

fn kata_exec_error(err: KataExecError, strategy_id: Uuid) -> McpError {
    tracing::warn!(
        strategy_id = %strategy_id,
        error = %err,
        "eval_python: kata exec error",
    );
    kata_exec_to_mcp_err(err)
}
