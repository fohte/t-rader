//! `eval_indicator` tool の inner method 実装。
//!
//! 戦略 scope (優先) → global scope の順に同名 indicator を解決し、`input_schema` で
//! 引数を validation した上で、Kata 上の exec Pod に code と `{"args": <args>}` を渡して
//! 実行する。stdout 最終行を JSON parse し、`output_schema` で validation した結果を返す。
//!
//! sandbox による拒否 (network / subprocess / fs write) は `eval_python` と同じく
//! `exit_code != 0` + `stderr` で透過する。MCP エラーには変換しない。

use std::time::Duration;

use rmcp::ErrorData as McpError;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::entities::custom_indicator;
use crate::kata_exec::{ExecRequest, KataExecError};
use crate::services::custom_indicators::resolve_indicator;

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
        session_strategy_id: Uuid,
        params: EvalIndicatorParams,
    ) -> Result<EvalIndicatorResult, McpError> {
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

        let indicator = resolve_indicator(&self.db, session_strategy_id, name)
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

        let executor = self
            .kata_executor
            .as_ref()
            .ok_or_else(|| internal_error("kata executor is not configured"))?;

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
    indicator: &custom_indicator::Model,
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

fn kata_exec_error(
    err: KataExecError,
    strategy_id: Uuid,
    indicator: &custom_indicator::Model,
) -> McpError {
    tracing::warn!(
        strategy_id = %strategy_id,
        indicator_id = %indicator.indicator_id,
        error = %err,
        "eval_indicator: kata exec error",
    );
    kata_exec_to_mcp_err(err)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rstest::rstest;
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{custom_indicator, strategy};
    use crate::kata_exec::{ExecResult, FakeKataExecutor, SharedKataExecutor};
    use crate::services::custom_indicators::{SCOPE_GLOBAL, SCOPE_STRATEGY};
    use crate::testing::create_test_db;

    use super::super::StrategyServer;
    use super::super::dto::{EvalIndicatorParams, EvalIndicatorResult};
    use super::{EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_TIMEOUT_SECS};

    async fn insert_strategy(db: &sea_orm::DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.into()),
            description: Set(None),
            sort_order: Set(0),
            agents_md: NotSet,
            skills: NotSet,
            agent_graph: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert strategy");
        id
    }

    async fn insert_indicator(
        db: &sea_orm::DatabaseConnection,
        scope: &str,
        strategy_id: Option<Uuid>,
        name: &str,
        code: &str,
        input_schema: serde_json::Value,
        output_schema: serde_json::Value,
    ) -> custom_indicator::Model {
        custom_indicator::ActiveModel {
            indicator_id: Set(Uuid::new_v4()),
            name: Set(name.into()),
            scope: Set(scope.into()),
            strategy_id: Set(strategy_id),
            code: Set(code.into()),
            input_schema: Set(input_schema),
            output_schema: Set(output_schema),
            description: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert indicator")
    }

    fn build_server(
        db: sea_orm::DatabaseConnection,
        executor: Arc<FakeKataExecutor>,
    ) -> (StrategyServer, SharedKataExecutor) {
        let shared: SharedKataExecutor = executor;
        let server = StrategyServer::new(db, None).with_kata_executor(Some(shared.clone()));
        (server, shared)
    }

    fn params(name: &str, args: serde_json::Value) -> EvalIndicatorParams {
        EvalIndicatorParams {
            name: name.into(),
            args,
            timeout_secs: None,
            max_output_bytes: None,
        }
    }

    #[sqlx::test(migrations = false)]
    async fn eval_indicator_resolves_and_runs(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let ind = insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "print('{\"value\": 42}')",
            json!({"type": "object", "properties": {"period": {"type": "integer"}}, "required": ["period"]}),
            json!({"type": "object", "properties": {"value": {"type": "number"}}, "required": ["value"]}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: "{\"value\": 42}\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }))
            .await;
        let (server, _shared) = build_server(db, executor.clone());

        let out = server
            .eval_indicator_inner(sid, params("rsi", json!({"period": 14})))
            .await
            .expect("eval");

        assert_eq!(
            out,
            EvalIndicatorResult {
                indicator_id: ind.indicator_id,
                scope: SCOPE_GLOBAL.into(),
                output: Some(json!({"value": 42})),
                stdout: "{\"value\": 42}\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }
        );
        let recorded = executor.requests.lock().await;
        assert_eq!(
            recorded.as_slice(),
            &[crate::kata_exec::ExecRequest {
                code: "print('{\"value\": 42}')".into(),
                stdin: Some(r#"{"args":{"period":14}}"#.into()),
                timeout: None,
                max_output_bytes: None,
            }],
        );
    }

    #[sqlx::test(migrations = false)]
    async fn eval_indicator_prefers_strategy_scope(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let _global = insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "print('{}')",
            json!({"type": "object"}),
            json!({"type": "object"}),
        )
        .await;
        let strategy_scoped = insert_indicator(
            &db,
            SCOPE_STRATEGY,
            Some(sid),
            "rsi",
            "print('{\"from\": \"strategy\"}')",
            json!({"type": "object"}),
            json!({"type": "object"}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: "{\"from\": \"strategy\"}\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }))
            .await;
        let (server, _shared) = build_server(db, executor.clone());

        let out = server
            .eval_indicator_inner(sid, params("rsi", json!({})))
            .await
            .expect("eval");
        assert_eq!(
            out,
            EvalIndicatorResult {
                indicator_id: strategy_scoped.indicator_id,
                scope: SCOPE_STRATEGY.into(),
                output: Some(json!({"from": "strategy"})),
                stdout: "{\"from\": \"strategy\"}\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }
        );
    }

    /// 戦略 A の session から戦略 B 専用 indicator は見えない (resolve 段で not found)。
    #[sqlx::test(migrations = false)]
    async fn eval_indicator_rejects_cross_strategy_scope(pool: PgPool) {
        let db = create_test_db(pool).await;
        let s_a = insert_strategy(&db, "a").await;
        let s_b = insert_strategy(&db, "b").await;
        insert_indicator(
            &db,
            SCOPE_STRATEGY,
            Some(s_b),
            "only-b",
            "print('{}')",
            json!({"type": "object"}),
            json!({"type": "object"}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _shared) = build_server(db, executor.clone());

        let err = server
            .eval_indicator_inner(s_a, params("only-b", json!({})))
            .await
            .expect_err("expected not found");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::RESOURCE_NOT_FOUND,
                "indicator 'only-b' not found",
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    #[sqlx::test(migrations = false)]
    async fn eval_indicator_validates_input_args(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "print('{}')",
            json!({
                "type": "object",
                "properties": {"period": {"type": "integer"}},
                "required": ["period"],
            }),
            json!({"type": "object"}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _shared) = build_server(db, executor.clone());

        let err = server
            .eval_indicator_inner(sid, params("rsi", json!({"period": "not-int"})))
            .await
            .expect_err("expected validation error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "args do not match input_schema of indicator 'rsi': \
                    \"not-int\" is not of type \"integer\" at /period",
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    /// sandbox 拒否 (network / subprocess / fs write) は MCP エラーではなく
    /// `exit_code != 0` + `stderr` で透過する。
    #[sqlx::test(migrations = false)]
    async fn eval_indicator_passes_through_sandbox_rejection(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        let ind = insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "import urllib.request; urllib.request.urlopen('http://x')",
            json!({"type": "object"}),
            json!({"type": "object"}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: String::new(),
                stderr: "PermissionError: network access denied".into(),
                exit_code: 1,
            }))
            .await;
        let (server, _shared) = build_server(db, executor.clone());

        let out = server
            .eval_indicator_inner(sid, params("rsi", json!({})))
            .await
            .expect("rejection is conveyed as result, not error");
        assert_eq!(
            out,
            EvalIndicatorResult {
                indicator_id: ind.indicator_id,
                scope: SCOPE_GLOBAL.into(),
                output: None,
                stdout: String::new(),
                stderr: "PermissionError: network access denied".into(),
                exit_code: 1,
            }
        );
    }

    #[sqlx::test(migrations = false)]
    async fn eval_indicator_rejects_invalid_output(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "print('not-json')",
            json!({"type": "object"}),
            json!({"type": "object", "required": ["value"]}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: "not-json\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }))
            .await;
        let (server, _shared) = build_server(db, executor.clone());

        let err = server
            .eval_indicator_inner(sid, params("rsi", json!({})))
            .await
            .expect_err("expected output parse error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "indicator 'rsi' last stdout line is not valid JSON: \
                    expected ident at line 1 column 2",
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn eval_indicator_rejects_output_schema_mismatch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "print('{}')",
            json!({"type": "object"}),
            json!({"type": "object", "required": ["value"]}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        executor
            .set_response(Ok(ExecResult {
                stdout: "{}\n".into(),
                stderr: String::new(),
                exit_code: 0,
            }))
            .await;
        let (server, _shared) = build_server(db, executor.clone());

        let err = server
            .eval_indicator_inner(sid, params("rsi", json!({})))
            .await
            .expect_err("expected output schema mismatch");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "output does not match output_schema of indicator 'rsi': \
                    \"value\" is a required property at ",
            ),
        );
    }

    /// indicator の input_schema が JSON Schema として壊れていた場合は operator 側の
    /// 問題なので `invalid_params` ではなく `internal_error` を返す。caller (LLM) に
    /// 「args が悪い」と誤認させない。
    #[sqlx::test(migrations = false)]
    async fn eval_indicator_reports_broken_input_schema_as_internal(pool: PgPool) {
        let db = create_test_db(pool).await;
        let sid = insert_strategy(&db, "s").await;
        insert_indicator(
            &db,
            SCOPE_GLOBAL,
            None,
            "rsi",
            "print('{}')",
            json!({"type": "not-a-real-type"}),
            json!({"type": "object"}),
        )
        .await;

        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _shared) = build_server(db, executor.clone());

        let err = server
            .eval_indicator_inner(sid, params("rsi", json!({})))
            .await
            .expect_err("expected internal error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "indicator 'rsi' has invalid input_schema in storage: \
                    \"not-a-real-type\" is not valid under any of the schemas \
                    listed in the 'anyOf' keyword",
            ),
        );
        assert!(executor.requests.lock().await.is_empty());
    }

    /// `sqlx::test` は `#[rstest]` と共存できないため `MockDatabase` + `tokio::test`。
    #[rstest]
    #[case::empty_name("", None, None, "name must not be empty".to_string())]
    #[case::excessive_timeout(
        "rsi",
        Some(EXEC_MAX_TIMEOUT_SECS + 1),
        None,
        format!("timeout_secs exceeds maximum of {EXEC_MAX_TIMEOUT_SECS}"),
    )]
    #[case::excessive_output(
        "rsi",
        None,
        Some(EXEC_MAX_OUTPUT_BYTES + 1),
        format!("max_output_bytes exceeds maximum of {EXEC_MAX_OUTPUT_BYTES}"),
    )]
    #[tokio::test]
    async fn eval_indicator_rejects_invalid_params(
        #[case] name: &str,
        #[case] timeout_secs: Option<u32>,
        #[case] max_output_bytes: Option<u32>,
        #[case] expected_msg: String,
    ) {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let sid = Uuid::new_v4();
        let executor = Arc::new(FakeKataExecutor::new());
        let (server, _shared) = build_server(db, executor.clone());

        let err = server
            .eval_indicator_inner(
                sid,
                EvalIndicatorParams {
                    name: name.into(),
                    args: json!({}),
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
}
