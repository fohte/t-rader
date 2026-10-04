//! 戦略実行 MCP server の tool 実装。各戦略の t-rader-agent から呼ばれる。
//! 戦略境界の保証の仕組みと各 tool の契約は docs/mcp.md 参照。
//!
//! 本モジュールは `StrategyServer` の構造体定義と、戦略境界・エラー変換などドメイン横断の
//! ヘルパを担う。

pub(super) mod annotations;
pub(super) mod comments;
pub(super) mod data;
mod dependencies;
pub(super) mod dto;
pub(super) mod eval;
pub(super) mod eval_indicator;
pub(super) mod evidence;
pub(super) mod fin_summary;
mod graph_dto;
pub(super) mod holdings;
pub(super) mod macro_indicator;
pub(super) mod margin;
pub(super) mod media;
pub(super) mod news;
pub(super) mod notes;
pub(super) mod portfolio;
pub(super) mod prediction_stats;
pub(super) mod predictions;
pub(super) mod ref_terms;
pub(super) mod refs;
pub(super) mod risk_check;
mod serde_helpers;
pub(super) mod short_ratio;
pub(super) mod short_sale_report;
pub(super) mod stock_groups;
pub(super) mod stock_registration;
pub(super) mod strategy_earnings_targets;
mod tool_router;
pub(super) mod trades;
pub(super) mod valuation;
pub(super) mod web_search;

use std::collections::BTreeMap;

use core_application::kata_exec::{KataExecError, SharedKataExecutor};
use core_application::llm_client::{LlmClientError as LiteLlmError, SharedLlmClient};
use core_application::persistence::PersistenceError;
use core_application::strategy_scope::{StrategyScope, StrategyScopeError};
use rmcp::ErrorData as McpError;
use rmcp::service::{RequestContext, RoleServer};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use uuid::Uuid;

pub use dependencies::StrategyServerDependencies;

const DEFAULT_LIST_LIMIT: u64 = 50;
const MAX_LIST_LIMIT: u64 = 200;

/// 戦略 Agent からの書き込み時に記録する actor 種別。
/// DB の CHECK 制約で `"human"` / `"llm"` のみ許容されているため `"llm"` を用いる。
pub(super) const STRATEGY_AGENT_ACTOR: &str = "llm";

const STRATEGY_ID_HEADER: &str = "x-strategy-id";
/// `x-execution-id` ヘッダ名。agent は `{a2a_task_id}:{step_id}` 形式の値を MCP tool 呼び出し
/// ごとに送る (`step_id` は agent 内の実行ステップ 1 件を指す不透明な文字列、`a2a_task_id` は
/// resume のたびに新しくなる実行 (試行) の id)。`a2a_task_id` をそのままキーにすると resume の
/// たびに別実行扱いになるため、`note.execution_id` には `step_id` 部分のみを保持し、
/// `annotation.execution_step_id` / `annotation.execution_task_id` には両者を分けて保持する。
/// 保存時に外部キーは検証しない。レビュー却下時は実行ステップ ID から親の戦略タスクを解決する。
/// 呼び出し元の `a2a_task_id` が現在アクティブな試行かどうかは検査しない。
/// `strategy_task.deadline_at` 超過による Failed 確定 (scheduler の照合 job) は
/// agent 側の実行を cancel しないため、resume 後も旧試行の agent プロセスが生存して
/// 呼び出しを送ってくると、新しい試行が書いた内容を上書き/削除しうる。
const EXECUTION_ID_HEADER: &str = "x-execution-id";
const TOOL_MODELS_HEADER: &str = "x-tool-models";

pub(super) const DEFAULT_ANNOTATION_STATUS: &str = "unread";

/// exec Pod に Python コード / indicator を渡す tool 群で共通の制限値。
/// 個別 tool で上書きしないこと。MCP 層と Pod 層の二重で適用される。
pub(super) const EXEC_MAX_TIMEOUT_SECS: u32 = 60;
pub(super) const EXEC_MAX_OUTPUT_BYTES: u32 = 1024 * 1024;
pub(super) const EXEC_MAX_STDIN_BYTES: usize = 256 * 1024;

/// `Option<u32>` の上限チェック。`0` も拒否する (executor 側で意味を持たないため)。
pub(super) fn check_exec_upper_bound(
    name: &str,
    value: Option<u32>,
    max: u32,
) -> Result<(), McpError> {
    let Some(v) = value else { return Ok(()) };
    if v == 0 {
        return Err(invalid_params(format!("{name} must be > 0")));
    }
    if v > max {
        return Err(invalid_params(format!("{name} exceeds maximum of {max}")));
    }
    Ok(())
}

/// `KataExecError` の MCP エラー変換。tracing は呼び出し側で行うこと
/// (tool 固有のコンテキストフィールドを残せるため)。
pub(super) fn kata_exec_to_mcp_err(err: KataExecError) -> McpError {
    match err {
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

/// `LiteLlmError` の MCP エラー変換。tracing は呼び出し側の strategy_id を残せるよう
/// 呼び出し元 (`query_media_inner` 等) で行う。
pub(super) fn litellm_error_to_mcp(err: LiteLlmError) -> McpError {
    match err {
        LiteLlmError::Network(msg) => internal_error(format!("litellm network error: {msg}")),
        LiteLlmError::Api { status, message } => {
            internal_error(format!("litellm api error (status {status}): {message}"))
        }
        LiteLlmError::Parse(msg) => {
            internal_error(format!("failed to parse litellm response: {msg}"))
        }
        LiteLlmError::Init(msg) => internal_error(format!("litellm client init error: {msg}")),
    }
}

#[derive(Clone)]
pub struct StrategyServer {
    pub(super) dependencies: StrategyServerDependencies,
}

impl StrategyServer {
    pub fn new(dependencies: StrategyServerDependencies) -> Self {
        Self { dependencies }
    }

    pub fn with_kata_executor(mut self, kata_executor: Option<SharedKataExecutor>) -> Self {
        self.dependencies.kata_executor = kata_executor;
        self
    }

    pub fn with_litellm_client(mut self, litellm_client: Option<SharedLlmClient>) -> Self {
        self.dependencies.llm_client = litellm_client;
        self
    }

    /// `KATA_EXEC_API_URL` が未設定だと executor は `None` のまま起動する。
    pub(super) fn kata_executor(&self) -> Result<&SharedKataExecutor, McpError> {
        self.dependencies
            .kata_executor
            .as_ref()
            .ok_or_else(|| internal_error("kata executor is not configured"))
    }

    pub(super) async fn strategy_scope_from_ctx(
        &self,
        ctx: &RequestContext<RoleServer>,
    ) -> Result<StrategyScope, McpError> {
        let id = strategy_id_from_ctx(ctx)?;
        self.dependencies
            .strategy_scope
            .verify(id)
            .await
            .map_err(|error| match error {
                StrategyScopeError::NotFound(id) => {
                    invalid_params(format!("strategy {id} not found"))
                }
                StrategyScopeError::Source(
                    core_application::strategy_scope::StrategyScopeSourceError::QueryFailed(
                        message,
                    ),
                ) => {
                    tracing::error!(error = %message, "strategy mcp db error");
                    internal_error(format!("database error: {message}"))
                }
            })
    }
}

// === ヘルパー / エラーマッピング ===

pub(super) fn internal_error(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::internal_error(msg, None)
}

pub(super) fn invalid_params(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::invalid_params(msg, None)
}

pub(super) fn internal_failure(err: &str) -> McpError {
    tracing::error!(error = %err, "strategy mcp internal failure");
    internal_error(format!("database error: {err}"))
}

pub(super) fn trade_error(error: core_application::trade::TradeUseCaseError) -> McpError {
    tracing::error!(error = %error, "strategy mcp trade operation failed");
    internal_error(format!("database error: {error}"))
}

pub(super) fn financial_summary_error(
    error: core_application::financial_summary::FinancialSummaryUseCaseError,
) -> McpError {
    tracing::error!(error = %error, "strategy mcp db error");
    internal_error(format!("database error: {error}"))
}

pub(super) fn valuation_error(
    error: core_application::valuation::ValuationUseCaseError,
) -> McpError {
    tracing::error!(error = %error, "strategy mcp db error");
    internal_error(format!("database error: {error}"))
}

pub(super) fn strategy_use_case_error_to_mcp(
    error: core_application::strategy::StrategyUseCaseError,
) -> McpError {
    match error {
        core_application::strategy::StrategyUseCaseError::Validation(message) => {
            invalid_params(message)
        }
        core_application::strategy::StrategyUseCaseError::NotFound(id) => {
            invalid_params(format!("strategy {id} not found"))
        }
        other => {
            tracing::error!(error = %other, "strategy mcp strategy operation failed");
            internal_error(format!("strategy operation failed: {other}"))
        }
    }
}

pub(super) fn clamp_limit(limit: Option<u32>) -> u64 {
    let value = limit.map(u64::from).unwrap_or(DEFAULT_LIST_LIMIT);
    value.clamp(1, MAX_LIST_LIMIT)
}

/// 4 桁の銘柄コードであることを検証する。J-Quants の 5 桁コードとの対応関係が
/// 明記されていない tool 群 (`read_shareholding_structure` / `read_short_sale_reports`) が共有する。
pub(super) fn validate_symbol(symbol: &str) -> Result<(), McpError> {
    reject_foreign_stock_id(symbol)?;
    if symbol.len() == 4 && symbol.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(invalid_params(format!(
            "symbol must be a 4-digit stock code, got {symbol:?}"
        )))
    }
}

pub(super) fn reject_foreign_stock_id(stock_id: &str) -> Result<(), McpError> {
    if core_domain::stock_id::has_country_prefix(stock_id) {
        Err(invalid_params(
            "only Japanese stocks are supported by this tool",
        ))
    } else {
        Ok(())
    }
}

/// `code LIKE 'symbol%'` は Postgres のデフォルト照合順序 (en_US.utf8) では既存の plain
/// B-tree index を使えず毎回フルスキャンになるため、同じ絞り込みを range 条件で表現する。
/// `code` は検証済みの 4 桁 `symbol` + 1 桁の 5 桁数字文字列なので、末尾に `'0'`〜`'9'` の
/// 範囲を与えれば同じ index でカバーできる。
pub(super) fn code_range(symbol: &str) -> (String, String) {
    (format!("{symbol}0"), format!("{symbol}9"))
}

fn strategy_id_from_headers(headers: &axum::http::HeaderMap) -> Result<Uuid, McpError> {
    let header = headers.get(STRATEGY_ID_HEADER).ok_or_else(|| {
        invalid_params(format!(
            "missing {STRATEGY_ID_HEADER} header on strategy mcp request"
        ))
    })?;
    let raw = header
        .to_str()
        .map_err(|_| invalid_params(format!("{STRATEGY_ID_HEADER} header is not valid ASCII")))?;
    Uuid::parse_str(raw.trim()).map_err(|_| {
        invalid_params(format!(
            "{STRATEGY_ID_HEADER} header is not a valid uuid: {raw}"
        ))
    })
}

fn strategy_id_from_ctx(ctx: &RequestContext<RoleServer>) -> Result<Uuid, McpError> {
    let parts = ctx
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| internal_error("missing http parts in mcp request context"))?;
    strategy_id_from_headers(&parts.headers)
}

/// `x-execution-id` ヘッダから実行 ID を取り出す。任意ヘッダなので `strategy_id_from_headers`
/// と異なりエラーにはせず、欠落・非 ASCII・空白のみの値はすべて `None` として扱う。
fn execution_id_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(EXECUTION_ID_HEADER)?.to_str().ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn execution_id_from_ctx(ctx: &RequestContext<RoleServer>) -> Option<String> {
    let parts = ctx.extensions.get::<axum::http::request::Parts>()?;
    execution_id_from_headers(&parts.headers)
}

fn tool_model_from_headers(
    headers: &axum::http::HeaderMap,
    tool_name: &str,
) -> Result<String, McpError> {
    let header = headers
        .get(TOOL_MODELS_HEADER)
        .ok_or_else(|| internal_error(format!("missing {TOOL_MODELS_HEADER} header")))?;
    let raw = header
        .to_str()
        .map_err(|_| invalid_params(format!("{TOOL_MODELS_HEADER} header is not valid ASCII")))?;
    let tool_models: BTreeMap<String, String> = serde_json::from_str(raw)
        .map_err(|_| invalid_params(format!("{TOOL_MODELS_HEADER} header is not valid JSON")))?;
    tool_models
        .get(tool_name)
        .cloned()
        .ok_or_else(|| internal_error(format!("tool model for {tool_name} is not configured")))
}

fn tool_model_from_ctx(
    ctx: &RequestContext<RoleServer>,
    tool_name: &str,
) -> Result<String, McpError> {
    let parts = ctx
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| internal_error("missing http parts in mcp request context"))?;
    tool_model_from_headers(&parts.headers, tool_name)
}

/// `x-execution-id` ヘッダ値 (`{a2a_task_id}:{step_id}`) から `step_id` を取り出す。
/// FK を持たない理由は `backend/crates/entrypoints/agent-mcp/src/strategy/evidence.rs` を参照。
fn execution_step_id_from_execution_id(execution_id: &str) -> Option<Uuid> {
    let (_, step_id) = execution_id.rsplit_once(':')?;
    Uuid::parse_str(step_id).ok()
}

fn execution_step_id_from_ctx(ctx: &RequestContext<RoleServer>) -> Option<Uuid> {
    execution_id_from_ctx(ctx).and_then(|id| execution_step_id_from_execution_id(&id))
}

/// `x-execution-id` ヘッダ値 (`{a2a_task_id}:{step_id}`) から `a2a_task_id` 部分を取り出す。
fn execution_task_id_from_execution_id(execution_id: &str) -> Option<&str> {
    let (task_id, _) = execution_id.rsplit_once(':')?;
    if task_id.is_empty() {
        None
    } else {
        Some(task_id)
    }
}

fn execution_task_id_from_ctx(ctx: &RequestContext<RoleServer>) -> Option<String> {
    let execution_id = execution_id_from_ctx(ctx)?;
    execution_task_id_from_execution_id(&execution_id).map(str::to_string)
}

pub(super) fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or_else(|| {
        tracing::warn!(value = %d, "decimal value out of f64 range; coerced to 0.0");
        0.0
    })
}

/// `PersistenceError` の MCP エラー変換。
pub(super) fn persistence_error_to_mcp(error: PersistenceError) -> McpError {
    match error {
        PersistenceError::Database(message) => internal_failure(&message),
        PersistenceError::MissingReference(_) => {
            invalid_params("referenced resource does not exist")
        }
        PersistenceError::Conflict(_) => internal_error("conflict: resource already exists"),
        PersistenceError::ConstraintViolation(_) => {
            invalid_params("value violates database constraint")
        }
        PersistenceError::RecordNotUpdated(_) => internal_error("not found: resource not found"),
    }
}

/// 戦略の未使用投資可能額 (投資可能額 + 実現損益 - 取得原価)。投資可能額が記録されて
/// いなければ `None`。
pub(super) fn unused_investable_amount(
    investable_amount: Option<Decimal>,
    realized_pnl: Decimal,
    cost_basis: Decimal,
) -> Option<Decimal> {
    investable_amount.map(|amount| amount + realized_pnl - cost_basis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::default(None, 50)]
    #[case::custom(Some(10), 10)]
    #[case::zero_floors_to_one(Some(0), 1)]
    #[case::over_max_caps(Some(10_000), 200)]
    fn clamps_limit(#[case] input: Option<u32>, #[case] expected: u64) {
        assert_eq!(clamp_limit(input), expected);
    }

    fn header_map_with(name: &'static str, value: Option<&[u8]>) -> axum::http::HeaderMap {
        let mut h = axum::http::HeaderMap::new();
        if let Some(v) = value {
            h.insert(
                name,
                axum::http::HeaderValue::from_bytes(v).expect("header value"),
            );
        }
        h
    }

    fn headers_with(strategy: Option<&str>) -> axum::http::HeaderMap {
        header_map_with(STRATEGY_ID_HEADER, strategy.map(str::as_bytes))
    }

    fn execution_headers_with(value: Option<&[u8]>) -> axum::http::HeaderMap {
        header_map_with(EXECUTION_ID_HEADER, value)
    }

    fn tool_models_headers_with(value: Option<&[u8]>) -> axum::http::HeaderMap {
        header_map_with(TOOL_MODELS_HEADER, value)
    }

    #[rstest]
    #[case::search_web("search_web", "example-model-search")]
    #[case::query_media("query_media", "example-model-media")]
    fn tool_model_from_headers_returns_the_value_for_the_requested_tool(
        #[case] tool_name: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            tool_model_from_headers(
                &tool_models_headers_with(Some(
                    br#"{"search_web":"example-model-search","query_media":"example-model-media"}"#,
                )),
                tool_name,
            ),
            Ok(expected.to_string()),
        );
    }

    #[rstest]
    #[case::missing_header(
        None,
        "search_web",
        rmcp::model::ErrorCode::INTERNAL_ERROR,
        "missing x-tool-models header"
    )]
    #[case::missing_search_web(
        Some(br#"{"query_media":"example-model-media"}"#.as_slice()),
        "search_web",
        rmcp::model::ErrorCode::INTERNAL_ERROR,
        "tool model for search_web is not configured"
    )]
    #[case::missing_query_media(
        Some(br#"{"search_web":"example-model-search"}"#.as_slice()),
        "query_media",
        rmcp::model::ErrorCode::INTERNAL_ERROR,
        "tool model for query_media is not configured"
    )]
    #[case::invalid_json(
        Some(b"not-json".as_slice()),
        "search_web",
        rmcp::model::ErrorCode::INVALID_PARAMS,
        "x-tool-models header is not valid JSON"
    )]
    fn tool_model_from_headers_rejects_unavailable_configuration(
        #[case] header: Option<&[u8]>,
        #[case] tool_name: &str,
        #[case] expected_code: rmcp::model::ErrorCode,
        #[case] expected_message: &str,
    ) {
        let err = tool_model_from_headers(&tool_models_headers_with(header), tool_name)
            .expect_err("expected tool model configuration error");
        assert_eq!(
            (err.code, err.message.as_ref()),
            (expected_code, expected_message),
        );
    }

    #[test]
    fn strategy_id_from_headers_parses_uuid() {
        let id = Uuid::new_v4();
        let parsed = strategy_id_from_headers(&headers_with(Some(&id.to_string())))
            .expect("parsed strategy id");
        assert_eq!(parsed, id);
    }

    #[rstest]
    #[case::missing(None, "missing x-strategy-id header on strategy mcp request")]
    #[case::not_uuid(
        Some("not-a-uuid"),
        "x-strategy-id header is not a valid uuid: not-a-uuid"
    )]
    fn strategy_id_from_headers_rejects_invalid(
        #[case] header: Option<&str>,
        #[case] expected_message: &str,
    ) {
        let err = strategy_id_from_headers(&headers_with(header))
            .expect_err("expected invalid header to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert_eq!(err.message, expected_message);
    }

    #[rstest]
    #[case::missing(None, None)]
    #[case::empty(Some(b"".as_slice()), None)]
    #[case::whitespace_only(Some(b"   ".as_slice()), None)]
    #[case::non_ascii(Some([0xff, 0xfe].as_slice()), None)]
    #[case::valid(Some(b"exec-1".as_slice()), Some("exec-1"))]
    fn execution_id_from_headers_cases(
        #[case] header: Option<&[u8]>,
        #[case] expected: Option<&str>,
    ) {
        let result = execution_id_from_headers(&execution_headers_with(header));
        assert_eq!(result, expected.map(str::to_string));
    }

    #[rstest]
    #[case::valid(
        "a2a-task-1:550e8400-e29b-41d4-a716-446655440000",
        Some(uuid::uuid!("550e8400-e29b-41d4-a716-446655440000"))
    )]
    #[case::no_colon("no-colon-here", None)]
    #[case::not_uuid_after_colon("task:not-a-uuid", None)]
    fn execution_step_id_from_execution_id_cases(
        #[case] execution_id: &str,
        #[case] expected: Option<Uuid>,
    ) {
        assert_eq!(execution_step_id_from_execution_id(execution_id), expected);
    }

    #[rstest]
    #[case::valid("a2a-task-1:550e8400-e29b-41d4-a716-446655440000", Some("a2a-task-1"))]
    #[case::no_colon("no-colon-here", None)]
    #[case::empty_task_id_part(":550e8400-e29b-41d4-a716-446655440000", None)]
    fn execution_task_id_from_execution_id_cases(
        #[case] execution_id: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(execution_task_id_from_execution_id(execution_id), expected);
    }
}
