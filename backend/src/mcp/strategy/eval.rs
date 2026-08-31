//! `eval_python` tool の inner method 実装。
//!
//! Python コードを Kata Containers 上の exec Pod で 1 回実行し、stdout/stderr/exit_code
//! を返す。MCP 層では timeout / 出力サイズ / code / stdin のバイト数上限を Pod 起動前に
//! 検査し、超過時は invalid_params を返す。

use std::time::Duration;

use rmcp::ErrorData as McpError;
use uuid::Uuid;

use crate::kata_exec::{ExecRequest, KataExecError};

use super::dto::{EvalPythonParams, EvalPythonResult};
use super::{
    EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_STDIN_BYTES, EXEC_MAX_TIMEOUT_SECS, StrategyServer,
    check_exec_upper_bound, internal_error, invalid_params, kata_exec_to_mcp_err,
};

/// MCP 層で許容する Python コード本体のサイズ上限 (バイト)。
pub(super) const MAX_CODE_BYTES: usize = 64 * 1024;

impl StrategyServer {
    pub(crate) async fn eval_python_inner(
        &self,
        session_strategy_id: Uuid,
        params: EvalPythonParams,
    ) -> Result<EvalPythonResult, McpError> {
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

        let executor = self
            .kata_executor
            .as_ref()
            .ok_or_else(|| internal_error("kata executor is not configured"))?;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rstest::rstest;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use uuid::Uuid;

    use crate::kata_exec::{ExecResult, FakeKataExecutor, KataExecError, SharedKataExecutor};

    use super::super::StrategyServer;
    use super::super::dto::{EvalPythonParams, EvalPythonResult};
    use super::*;

    fn mock_db() -> sea_orm::DatabaseConnection {
        MockDatabase::new(DatabaseBackend::Postgres).into_connection()
    }

    fn build_server(executor: Arc<FakeKataExecutor>) -> (StrategyServer, SharedKataExecutor) {
        let shared: SharedKataExecutor = executor;
        let server = StrategyServer::new(mock_db(), None).with_kata_executor(Some(shared.clone()));
        (server, shared)
    }

    fn params(code: &str) -> EvalPythonParams {
        EvalPythonParams {
            code: code.into(),
            stdin: None,
            timeout_secs: None,
            max_output_bytes: None,
        }
    }

    #[tokio::test]
    async fn eval_python_returns_executor_result() {
        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: "2\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }))
            .await;
        let (server, _shared) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let out = server
            .eval_python_inner(sid, params("print(1+1)"))
            .await
            .expect("eval");

        assert_eq!(
            out,
            EvalPythonResult {
                stdout: "2\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }
        );
        let recorded = executor.requests.lock().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].code, "print(1+1)");
    }

    #[rstest]
    #[case::empty_code(
        String::new(),
        None,
        None,
        "code must not be empty".to_string(),
    )]
    #[case::oversized_code(
        "a".repeat(MAX_CODE_BYTES + 1),
        None,
        None,
        format!("code exceeds {MAX_CODE_BYTES} bytes"),
    )]
    #[case::excessive_timeout(
        "print(1)".into(),
        Some(EXEC_MAX_TIMEOUT_SECS + 1),
        None,
        format!("timeout_secs exceeds maximum of {EXEC_MAX_TIMEOUT_SECS}"),
    )]
    #[case::excessive_output(
        "print(1)".into(),
        None,
        Some(EXEC_MAX_OUTPUT_BYTES + 1),
        format!("max_output_bytes exceeds maximum of {EXEC_MAX_OUTPUT_BYTES}"),
    )]
    #[tokio::test]
    async fn eval_python_rejects_invalid_input(
        #[case] code: String,
        #[case] timeout_secs: Option<u32>,
        #[case] max_output_bytes: Option<u32>,
        #[case] expected_msg: String,
    ) {
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(
                sid,
                EvalPythonParams {
                    code,
                    stdin: None,
                    timeout_secs,
                    max_output_bytes,
                },
            )
            .await
            .expect_err("expected validation error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                expected_msg.as_str()
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    #[rstest]
    #[case::timeout(
        KataExecError::Timeout(Duration::from_secs(5)),
        format!("execution timed out after {:?}", Duration::from_secs(5)),
    )]
    #[case::output_too_large(
        KataExecError::OutputTooLarge { limit: 1024 },
        "output exceeded 1024 bytes".to_string(),
    )]
    #[tokio::test]
    async fn eval_python_maps_executor_error_to_invalid_params(
        #[case] executor_error: KataExecError,
        #[case] expected_msg: String,
    ) {
        let executor = Arc::new(FakeKataExecutor::new());
        executor.set_response(Err(executor_error)).await;
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(sid, params("print(1)"))
            .await
            .expect_err("expected mapped error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                expected_msg.as_str()
            ),
        );
    }

    /// sandbox 拒否は MCP エラーではなく ExecResult (exit_code != 0) として透過させる。
    /// KataExecError 経路と混同しないこと。
    #[tokio::test]
    async fn eval_python_passes_through_sandbox_rejection() {
        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: String::new(),
                stderr: "PermissionError: network access denied".into(),
                exit_code: 1,
            }))
            .await;
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let out = server
            .eval_python_inner(
                sid,
                params("import urllib.request; urllib.request.urlopen('http://x')"),
            )
            .await
            .expect("rejection is conveyed as result, not error");
        assert_eq!(
            out,
            EvalPythonResult {
                stdout: String::new(),
                stderr: "PermissionError: network access denied".into(),
                exit_code: 1,
            },
        );
    }

    #[tokio::test]
    async fn eval_python_errors_when_executor_not_configured() {
        let server = StrategyServer::new(mock_db(), None);
        let sid = Uuid::new_v4();
        let err = server
            .eval_python_inner(sid, params("print(1)"))
            .await
            .expect_err("not configured");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "kata executor is not configured",
            ),
        );
    }
}
