use super::{EXEC_MAX_OUTPUT_BYTES, EXEC_MAX_TIMEOUT_SECS};

use std::sync::Arc;

use rstest::rstest;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use serde_json::json;
use uuid::Uuid;

use super::StrategyServer;
use super::dto::{EvalIndicatorParams, EvalIndicatorResult};
use super::tests_common::mock_db_with_strategy;
use core_application::custom_indicator::{SCOPE_GLOBAL, SCOPE_STRATEGY};
use core_application::kata_exec::{ExecRequest, ExecResult, FakeKataExecutor, SharedKataExecutor};
use gateway_postgres::entities::{custom_indicator, strategy};

async fn insert_strategy(db: &impl sea_orm::ConnectionTrait, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    strategy::ActiveModel {
        id: Set(id),
        name: Set(name.into()),
        description: Set(None),
        sort_order: Set(0),
        created_at: NotSet,
        updated_at: NotSet,
    }
    .insert(db)
    .await
    .expect("insert strategy");
    id
}

async fn insert_indicator(
    db: &impl sea_orm::ConnectionTrait,
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
    db: impl Into<gateway_postgres::DatabaseHandle>,
    executor: Arc<FakeKataExecutor>,
) -> (StrategyServer, SharedKataExecutor) {
    let shared: SharedKataExecutor = executor;
    let server = super::tests_common::build_server(db).with_kata_executor(Some(shared.clone()));
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

#[backend_test_macros::database_test]
async fn eval_indicator_resolves_and_runs(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(sid, params("rsi", json!({"period": 14})))
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
        &[ExecRequest {
            code: "print('{\"value\": 42}')".into(),
            stdin: Some(r#"{"args":{"period":14}}"#.into()),
            timeout: None,
            max_output_bytes: None,
        }],
    );
}

#[backend_test_macros::database_test]
async fn eval_indicator_prefers_strategy_scope(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(sid, params("rsi", json!({})))
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
#[backend_test_macros::database_test]
async fn eval_indicator_rejects_cross_strategy_scope(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(s_a, params("only-b", json!({})))
        .await
        .expect_err("expected not found");
    assert_eq!(
        err,
        rmcp::ErrorData::resource_not_found("indicator 'only-b' not found", None),
    );
    assert!(executor.requests.lock().await.is_empty());
}

#[backend_test_macros::database_test]
async fn eval_indicator_validates_input_args(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(sid, params("rsi", json!({"period": "not-int"})))
        .await
        .expect_err("expected validation error");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params(
            "args do not match input_schema of indicator 'rsi': \
                \"not-int\" is not of type \"integer\" at /period",
            None,
        ),
    );
    assert!(executor.requests.lock().await.is_empty());
}

/// sandbox 拒否 (network / subprocess / fs write) は MCP エラーではなく
/// `exit_code != 0` + `stderr` で透過する。
#[backend_test_macros::database_test]
async fn eval_indicator_passes_through_sandbox_rejection(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(sid, params("rsi", json!({})))
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

#[backend_test_macros::database_test]
async fn eval_indicator_rejects_invalid_output(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(sid, params("rsi", json!({})))
        .await
        .expect_err("expected output parse error");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params(
            "indicator 'rsi' last stdout line is not valid JSON: \
                expected ident at line 1 column 2",
            None,
        ),
    );
}

#[backend_test_macros::database_test]
async fn eval_indicator_rejects_output_schema_mismatch(db: gateway_postgres::DatabaseHandle) {
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
        .eval_indicator(sid, params("rsi", json!({})))
        .await
        .expect_err("expected output schema mismatch");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params(
            "output does not match output_schema of indicator 'rsi': \
                \"value\" is a required property at ",
            None,
        ),
    );
}

/// indicator の input_schema が JSON Schema として壊れていた場合は operator 側の
/// 問題なので `invalid_params` ではなく `internal_error` を返す。caller (LLM) に
/// 「args が悪い」と誤認させない。
#[backend_test_macros::database_test]
async fn eval_indicator_reports_broken_input_schema_as_internal(
    db: gateway_postgres::DatabaseHandle,
) {
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
        .eval_indicator(sid, params("rsi", json!({})))
        .await
        .expect_err("expected internal error");
    assert_eq!(
        err,
        rmcp::ErrorData::internal_error(
            "indicator 'rsi' has invalid input_schema in storage: \
                \"not-a-real-type\" is not valid under any of the schemas \
                listed in the 'anyOf' keyword",
            None,
        ),
    );
    assert!(executor.requests.lock().await.is_empty());
}

#[backend_test_macros::database_test]
async fn eval_indicator_errors_when_executor_not_configured(db: gateway_postgres::DatabaseHandle) {
    let sid = insert_strategy(&db, "s").await;
    insert_indicator(
        &db,
        SCOPE_GLOBAL,
        None,
        "rsi",
        "print('{}')",
        json!({"type": "object"}),
        json!({"type": "object"}),
    )
    .await;
    let server = super::tests_common::build_server(db);

    let err = server
        .eval_indicator(sid, params("rsi", json!({})))
        .await
        .expect_err("not configured");
    assert_eq!(
        err,
        rmcp::ErrorData::internal_error("kata executor is not configured", None),
    );
}

/// `database_test` は rstest の case 引数を扱わないため `MockDatabase` + `tokio::test`。
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
    let sid = Uuid::new_v4();
    let db = mock_db_with_strategy(sid);
    let executor = Arc::new(FakeKataExecutor::new());
    let (server, _shared) = build_server(db, executor.clone());

    let err = server
        .eval_indicator(
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
    assert_eq!(err, rmcp::ErrorData::invalid_params(expected_msg, None));
    assert!(executor.requests.lock().await.is_empty());
}
