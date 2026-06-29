use std::time::Duration;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::entities::custom_indicator;
use crate::error::AppError;
use crate::kata_exec::{ExecRequest, KataExecError, SharedKataExecutor};

pub const SCOPE_GLOBAL: &str = "global";
pub const SCOPE_STRATEGY: &str = "strategy";

/// MCP 層と HTTP preview 層の両方で適用する exec 上限。
/// MCP 側 (`backend/src/mcp/strategy/mod.rs`) と数値を揃えること。
pub const PREVIEW_MAX_TIMEOUT_SECS: u32 = 60;
pub const PREVIEW_MAX_OUTPUT_BYTES: u32 = 1024 * 1024;
pub const PREVIEW_MAX_STDIN_BYTES: usize = 256 * 1024;
pub const PREVIEW_MAX_CODE_BYTES: usize = 64 * 1024;

/// indicator code を 1 回だけ実行した結果。preview endpoint 用。
pub struct PreviewOutcome {
    pub output: Option<JsonValue>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct PreviewInput<'a> {
    pub code: &'a str,
    pub input_schema: &'a JsonValue,
    pub output_schema: &'a JsonValue,
    pub args: &'a JsonValue,
    pub timeout_secs: Option<u32>,
    pub max_output_bytes: Option<u32>,
}

/// indicator 行を保存せずに `eval_indicator` と同じ runtime で 1 回実行する。
///
/// validation 失敗 (input/output schema、入力サイズ等) は `AppError::Validation`、
/// sandbox 拒否は `exit_code != 0` + `stderr` でそのまま透過する。executor 未設定は
/// `ServiceUnavailable`、kata 側の障害は `Validation` (timeout 等) もしくは
/// `ServiceUnavailable` に振り分ける。
pub async fn run_preview(
    executor: &SharedKataExecutor,
    input: PreviewInput<'_>,
) -> Result<PreviewOutcome, AppError> {
    if input.code.is_empty() {
        return Err(AppError::Validation("code must not be empty".into()));
    }
    if input.code.len() > PREVIEW_MAX_CODE_BYTES {
        return Err(AppError::Validation(format!(
            "code exceeds {PREVIEW_MAX_CODE_BYTES} bytes"
        )));
    }
    check_upper_bound("timeout_secs", input.timeout_secs, PREVIEW_MAX_TIMEOUT_SECS)?;
    check_upper_bound(
        "max_output_bytes",
        input.max_output_bytes,
        PREVIEW_MAX_OUTPUT_BYTES,
    )?;

    validate_schema_instance("input_schema", input.input_schema, input.args)?;

    let stdin = serde_json::to_string(&serde_json::json!({ "args": input.args }))
        .map_err(|e| AppError::Validation(format!("failed to serialize args: {e}")))?;
    if stdin.len() > PREVIEW_MAX_STDIN_BYTES {
        return Err(AppError::Validation(format!(
            "args exceed {PREVIEW_MAX_STDIN_BYTES} bytes when serialized as JSON"
        )));
    }

    let request = ExecRequest {
        code: input.code.to_string(),
        stdin: Some(stdin),
        timeout: input.timeout_secs.map(|s| Duration::from_secs(s.into())),
        max_output_bytes: input.max_output_bytes.map(|m| m as usize),
    };

    let result = executor.run(request).await.map_err(kata_to_app_err)?;

    let mut stderr = result.stderr;
    // 出力 validation 失敗は HTTP 400 にしない。preview 経路では「スクリプトは
    // 動いたが output_schema に合っていない」状態を stdout/stderr 込みで返さないと
    // ユーザーが何を直すべきか判断できなくなる。MCP 経路 (eval_indicator) は LLM 向けで
    // 「コードを修正させる」ためにエラーで返すが、preview はユーザー向けの DX 優先。
    let output = if result.exit_code == 0 {
        match parse_and_validate_output(&result.stdout, input.output_schema) {
            Ok(val) => Some(val),
            Err(e) => {
                if !stderr.is_empty() {
                    stderr.push('\n');
                }
                stderr.push_str(&format!("Output validation error: {e}"));
                None
            }
        }
    } else {
        None
    };

    Ok(PreviewOutcome {
        output,
        stdout: result.stdout,
        stderr,
        exit_code: result.exit_code,
    })
}

fn check_upper_bound(name: &str, value: Option<u32>, max: u32) -> Result<(), AppError> {
    let Some(v) = value else { return Ok(()) };
    if v == 0 {
        return Err(AppError::Validation(format!("{name} must be > 0")));
    }
    if v > max {
        return Err(AppError::Validation(format!(
            "{name} exceeds maximum of {max}"
        )));
    }
    Ok(())
}

fn validate_schema_instance(
    label: &str,
    schema: &JsonValue,
    instance: &JsonValue,
) -> Result<(), AppError> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| AppError::Validation(format!("{label} is not a valid JSON Schema: {e}")))?;
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "value does not match {label}: {}",
            errors.join("; ")
        )))
    }
}

fn parse_and_validate_output(
    stdout: &str,
    output_schema: &JsonValue,
) -> Result<JsonValue, AppError> {
    let last_line = stdout.lines().rfind(|l| !l.trim().is_empty());
    let Some(raw) = last_line else {
        return Err(AppError::Validation(
            "indicator produced empty stdout; expected a JSON value on the last line".into(),
        ));
    };
    let parsed: JsonValue = serde_json::from_str(raw.trim())
        .map_err(|e| AppError::Validation(format!("last stdout line is not valid JSON: {e}")))?;
    validate_schema_instance("output_schema", output_schema, &parsed)?;
    Ok(parsed)
}

fn kata_to_app_err(err: KataExecError) -> AppError {
    match err {
        KataExecError::NotConfigured => {
            AppError::ServiceUnavailable("kata executor is not configured".into())
        }
        KataExecError::Timeout(d) => {
            AppError::Validation(format!("execution timed out after {d:?}"))
        }
        KataExecError::OutputTooLarge { limit } => {
            AppError::Validation(format!("output exceeded {limit} bytes"))
        }
        KataExecError::PodFailed(msg)
        | KataExecError::Api { message: msg, .. }
        | KataExecError::Network(msg)
        | KataExecError::Parse(msg)
        | KataExecError::Init(msg) => {
            tracing::error!("kata exec error: {msg}");
            AppError::ServiceUnavailable("indicator runtime is currently unavailable".into())
        }
    }
}

/// 戦略 scope の同名 indicator があれば優先し、無ければ global を返す
pub async fn resolve_indicator<C: ConnectionTrait>(
    conn: &C,
    strategy_id: Uuid,
    name: &str,
) -> Result<Option<custom_indicator::Model>, AppError> {
    let strategy_scoped = custom_indicator::Entity::find()
        .filter(custom_indicator::Column::Scope.eq(SCOPE_STRATEGY))
        .filter(custom_indicator::Column::StrategyId.eq(strategy_id))
        .filter(custom_indicator::Column::Name.eq(name))
        .one(conn)
        .await?;
    if strategy_scoped.is_some() {
        return Ok(strategy_scoped);
    }

    let global = custom_indicator::Entity::find()
        .filter(custom_indicator::Column::Scope.eq(SCOPE_GLOBAL))
        .filter(custom_indicator::Column::Name.eq(name))
        .one(conn)
        .await?;
    Ok(global)
}
