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
use super::{StrategyServer, ensure_strategy_match, internal_error, invalid_params};

/// MCP 層で許容する wall-clock timeout の上限 (秒)。
pub(super) const MAX_TIMEOUT_SECS: u32 = 60;
/// MCP 層で許容する出力サイズ上限 (バイト)。executor 側のデフォルトと同じ 1 MiB。
pub(super) const MAX_OUTPUT_BYTES: u32 = 1024 * 1024;
/// MCP 層で許容する Python コード本体のサイズ上限 (バイト)。
pub(super) const MAX_CODE_BYTES: usize = 64 * 1024;
/// MCP 層で許容する stdin のサイズ上限 (バイト)。
pub(super) const MAX_STDIN_BYTES: usize = 256 * 1024;

impl StrategyServer {
    pub(crate) async fn eval_python_inner(
        &self,
        session_strategy_id: Uuid,
        params: EvalPythonParams,
    ) -> Result<EvalPythonResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        if params.code.is_empty() {
            return Err(invalid_params("code must not be empty"));
        }
        if params.code.len() > MAX_CODE_BYTES {
            return Err(invalid_params(format!(
                "code exceeds {MAX_CODE_BYTES} bytes"
            )));
        }
        if let Some(stdin) = params.stdin.as_ref()
            && stdin.len() > MAX_STDIN_BYTES
        {
            return Err(invalid_params(format!(
                "stdin exceeds {MAX_STDIN_BYTES} bytes"
            )));
        }
        check_upper_bound("timeout_secs", params.timeout_secs, MAX_TIMEOUT_SECS)?;
        check_upper_bound(
            "max_output_bytes",
            params.max_output_bytes,
            MAX_OUTPUT_BYTES,
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

fn check_upper_bound(name: &str, value: Option<u32>, max: u32) -> Result<(), McpError> {
    let Some(v) = value else { return Ok(()) };
    if v == 0 {
        return Err(invalid_params(format!("{name} must be > 0")));
    }
    if v > max {
        return Err(invalid_params(format!("{name} exceeds maximum of {max}")));
    }
    Ok(())
}

fn kata_exec_error(err: KataExecError, strategy_id: Uuid) -> McpError {
    tracing::warn!(
        strategy_id = %strategy_id,
        error = %err,
        "eval_python: kata exec error",
    );
    match err {
        KataExecError::NotConfigured => internal_error("kata executor is not configured"),
        KataExecError::Timeout(d) => invalid_params(format!("execution timed out after {:?}", d)),
        KataExecError::OutputTooLarge { limit } => {
            invalid_params(format!("output exceeded {limit} bytes"))
        }
        KataExecError::PodFailed(msg) => internal_error(format!("exec pod failed: {msg}")),
        KataExecError::Api { status, message } => {
            internal_error(format!("kube api error (status {status}): {message}"))
        }
        KataExecError::Network(msg) => internal_error(format!("kube api network error: {msg}")),
        KataExecError::Parse(msg) => internal_error(format!("kube api parse error: {msg}")),
        KataExecError::Init(msg) => internal_error(format!("kata executor init error: {msg}")),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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

    fn params(strategy_id: Uuid, code: &str) -> EvalPythonParams {
        EvalPythonParams {
            strategy_id,
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
            .eval_python_inner(sid, params(sid, "print(1+1)"))
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

    #[tokio::test]
    async fn eval_python_rejects_strategy_mismatch() {
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _) = build_server(executor.clone());
        let session = Uuid::new_v4();
        let other = Uuid::new_v4();

        let err = server
            .eval_python_inner(session, params(other, "print(1)"))
            .await
            .expect_err("mismatch");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!(
                    "forbidden: strategy_id boundary violation (session={session}, arg={other})"
                )
                .as_str(),
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn eval_python_rejects_empty_code() {
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(sid, params(sid, ""))
            .await
            .expect_err("empty");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "code must not be empty"
            ),
        );
    }

    #[tokio::test]
    async fn eval_python_rejects_oversized_code() {
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();
        let huge = "a".repeat(MAX_CODE_BYTES + 1);

        let err = server
            .eval_python_inner(sid, params(sid, &huge))
            .await
            .expect_err("oversized");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("code exceeds {MAX_CODE_BYTES} bytes").as_str(),
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn eval_python_rejects_excessive_timeout() {
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(
                sid,
                EvalPythonParams {
                    timeout_secs: Some(MAX_TIMEOUT_SECS + 1),
                    ..params(sid, "print(1)")
                },
            )
            .await
            .expect_err("excessive timeout");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("timeout_secs exceeds maximum of {MAX_TIMEOUT_SECS}").as_str(),
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn eval_python_rejects_excessive_output_limit() {
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(
                sid,
                EvalPythonParams {
                    max_output_bytes: Some(MAX_OUTPUT_BYTES + 1),
                    ..params(sid, "print(1)")
                },
            )
            .await
            .expect_err("excessive output");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("max_output_bytes exceeds maximum of {MAX_OUTPUT_BYTES}").as_str(),
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn eval_python_maps_timeout_to_invalid_params() {
        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Err(KataExecError::Timeout(Duration::from_secs(5))))
            .await;
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(sid, params(sid, "while True: pass"))
            .await
            .expect_err("timeout");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("execution timed out after {:?}", Duration::from_secs(5)).as_str(),
            ),
        );
    }

    #[tokio::test]
    async fn eval_python_maps_output_too_large_to_invalid_params() {
        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Err(KataExecError::OutputTooLarge { limit: 1024 }))
            .await;
        let (server, _) = build_server(executor.clone());
        let sid = Uuid::new_v4();

        let err = server
            .eval_python_inner(sid, params(sid, "print('x' * 10_000_000)"))
            .await
            .expect_err("too large");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "output exceeded 1024 bytes",
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
                params(
                    sid,
                    "import urllib.request; urllib.request.urlopen('http://x')",
                ),
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
            .eval_python_inner(sid, params(sid, "print(1)"))
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
