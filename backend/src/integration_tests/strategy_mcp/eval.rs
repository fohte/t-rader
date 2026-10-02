use std::time::Duration;

use super::{EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_TIMEOUT_SECS, MAX_CODE_BYTES};

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

    fn mock_db(strategy_id: Uuid) -> sea_orm::DatabaseConnection {
        let row = std::collections::BTreeMap::from([(
            "id".to_string(),
            sea_orm::Value::from(strategy_id),
        )]);
        MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![row]])
            .into_connection()
    }

    fn build_server(executor: Arc<FakeKataExecutor>) -> (StrategyServer, SharedKataExecutor, Uuid) {
        let strategy_id = Uuid::new_v4();
        let shared: SharedKataExecutor = executor;
        let server = super::super::tests_common::build_server(mock_db(strategy_id))
            .with_kata_executor(Some(shared.clone()));
        (server, shared, strategy_id)
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
        let (server, _shared, strategy_id) = build_server(executor.clone());

        let out = server
            .eval_python(strategy_id, params("print(1+1)"))
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
        let (server, _, strategy_id) = build_server(executor.clone());

        let err = server
            .eval_python(
                strategy_id,
                EvalPythonParams {
                    code,
                    stdin: None,
                    timeout_secs,
                    max_output_bytes,
                },
            )
            .await
            .expect_err("expected validation error");
        assert_eq!(err, rmcp::ErrorData::invalid_params(expected_msg, None));
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
        let (server, _, strategy_id) = build_server(executor.clone());

        let err = server
            .eval_python(strategy_id, params("print(1)"))
            .await
            .expect_err("expected mapped error");
        assert_eq!(err, rmcp::ErrorData::invalid_params(expected_msg, None));
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
        let (server, _, strategy_id) = build_server(executor.clone());

        let out = server
            .eval_python(
                strategy_id,
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
        let strategy_id = Uuid::new_v4();
        let server = super::super::tests_common::build_server(mock_db(strategy_id));
        let err = server
            .eval_python(strategy_id, params("print(1)"))
            .await
            .expect_err("not configured");
        assert_eq!(
            err,
            rmcp::ErrorData::internal_error("kata executor is not configured", None),
        );
    }
}
