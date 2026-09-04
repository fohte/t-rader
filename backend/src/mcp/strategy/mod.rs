//! 戦略実行 MCP server の tool 実装
//!
//! 各戦略の t-rader-agent から呼ばれる。接続コンテキストに `x-strategy-id`
//! HTTP ヘッダで自身の strategy_id を持ち込み、全 tool はこの値のみを戦略境界として
//! 使う (tool 引数に strategy_id は含まれない)。さらに対象リソース (note / annotation)
//! の strategy_id と一致するかを Repository 層で二重検査する。
//!
//! tool 一覧:
//!
//! - `query_data`: 銘柄 + 期間で価格時系列を返す (DataProvider 経由)
//! - `write_note`: ノートを作成または更新する
//! - `read_note`: ノートを取得する
//! - `list_notes`: ノート一覧を返す
//! - `create_annotation`: アノテーションを作成する
//! - `read_annotations`: アノテーション一覧を返す
//! - `read_comments`: ノート / アノテーションに付いたレビューコメントを読み出す (resolved で絞り込み可)
//! - `resolve_comment`: レビューコメントを解決済み/未解決に切り替える
//! - `reply_comment`: レビューコメントに返信する
//! - `eval_python`: Python コードを exec Pod (Kata Containers) 上で実行する
//! - `add_interest`: 戦略の関心 (derived / origin=llm) を追加する
//! - `eval_indicator`: DB の indicator (戦略 scope 優先、無ければ global) を exec Pod 上で評価する
//! - `query_media`: 動画/音声 URL (YouTube 等) の内容を Gemini (LiteLLM 経由) でテキスト化する
//!
//! 実装はドメインごとに分割している:
//!
//! - `dto`: 各 tool の入出力スキーマ
//! - `notes`: ノート操作 (`write_note_inner` / `read_note_inner` / `list_notes_inner`)
//! - `annotations`: アノテーション操作 (`create_annotation_inner` / `read_annotations_inner`)
//! - `comments`: コメント操作 (`read_comments_inner` / `resolve_comment_inner` / `reply_comment_inner`)
//! - `data`: 価格データ取得 (`query_data_inner`)
//! - `eval`: Python 実行 (`eval_python_inner`)
//! - `interests`: 関心の追加 (`add_interest_inner`)
//! - `eval_indicator`: 永続化された indicator の評価 (`eval_indicator_inner`)
//! - `media`: 動画/音声 URL の Gemini によるテキスト化 (`query_media_inner`)
//! - `tool_router`: `#[tool_router]` 登録、ctx から strategy_id を取り出し `*_inner` に
//!   委譲する薄い tool wrapper、`#[tool_handler] impl ServerHandler`
//!   (`tool_router()` が生成する関連関数がモジュール private なため同居させている)
//!
//! 本モジュールは `StrategyServer` の構造体定義と、戦略境界・エラー変換などドメイン横断の
//! ヘルパを担う。

pub(super) mod annotations;
pub(super) mod comments;
pub(super) mod data;
pub(super) mod dto;
pub(super) mod eval;
pub(super) mod eval_indicator;
pub(super) mod interests;
pub(super) mod media;
pub(super) mod notes;
mod tool_router;

#[cfg(test)]
mod tests_common;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::service::{RequestContext, RoleServer};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::data_provider::{DataProviderError, DataProviderKind};
use crate::entities::{annotation, note, strategy};
use crate::kata_exec::SharedKataExecutor;
use crate::services::litellm_client::{LiteLlmClient, LiteLlmError};

const DEFAULT_LIST_LIMIT: u64 = 50;
const MAX_LIST_LIMIT: u64 = 200;

/// 戦略 Agent からの書き込み時に記録する actor 種別。
/// DB の CHECK 制約で `"human"` / `"llm"` のみ許容されているため `"llm"` を用いる。
pub(super) const STRATEGY_AGENT_ACTOR: &str = "llm";

const STRATEGY_ID_HEADER: &str = "x-strategy-id";
/// `x-execution-id` ヘッダ名。agent が A2A タスク実行のたびに送る値で、backend は同じ値を
/// `strategy_task.a2a_task_id` としても保持するが、`note.execution_id` 側に FK/join はなく
/// 単なる相関用の不透明な文字列として扱う。
const EXECUTION_ID_HEADER: &str = "x-execution-id";

pub(super) const DEFAULT_NOTE_STATUS: &str = "unread";
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
pub(super) fn kata_exec_to_mcp_err(err: crate::kata_exec::KataExecError) -> McpError {
    use crate::kata_exec::KataExecError;
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
    db: DatabaseConnection,
    data_provider: Option<Arc<DataProviderKind>>,
    pub(super) kata_executor: Option<SharedKataExecutor>,
    pub(super) litellm_client: Option<LiteLlmClient>,
}

impl StrategyServer {
    pub fn new(db: DatabaseConnection, data_provider: Option<Arc<DataProviderKind>>) -> Self {
        Self {
            db,
            data_provider,
            kata_executor: None,
            litellm_client: None,
        }
    }

    pub fn with_kata_executor(mut self, kata_executor: Option<SharedKataExecutor>) -> Self {
        self.kata_executor = kata_executor;
        self
    }

    pub fn with_litellm_client(mut self, litellm_client: Option<LiteLlmClient>) -> Self {
        self.litellm_client = litellm_client;
        self
    }
}

// === ヘルパー / エラーマッピング ===

pub(super) fn internal_error(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::internal_error(msg, None)
}

pub(super) fn invalid_params(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::invalid_params(msg, None)
}

pub(super) fn db_error(err: sea_orm::DbErr) -> McpError {
    tracing::error!(error = %err, "strategy mcp db error");
    internal_error(format!("database error: {err}"))
}

pub(super) fn data_provider_error(err: DataProviderError) -> McpError {
    tracing::warn!(error = %err, "strategy mcp data provider error");
    match err {
        DataProviderError::NotFound(msg) => {
            McpError::resource_not_found(format!("instrument not found: {msg}"), None)
        }
        other => internal_error(format!("data provider error: {other}")),
    }
}

pub(super) fn clamp_limit(limit: Option<u32>) -> u64 {
    let value = limit.map(u64::from).unwrap_or(DEFAULT_LIST_LIMIT);
    value.clamp(1, MAX_LIST_LIMIT)
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

pub(super) async fn fetch_note_owned_by(
    db: &DatabaseConnection,
    note_id: Uuid,
    expected: Uuid,
) -> Result<note::Model, McpError> {
    let row = note::Entity::find_by_id(note_id)
        .one(db)
        .await
        .map_err(db_error)?
        .ok_or_else(|| McpError::resource_not_found("note not found", None))?;
    if row.strategy_id != expected {
        return Err(invalid_params(format!(
            "forbidden: note {note_id} belongs to another strategy"
        )));
    }
    Ok(row)
}

pub(super) async fn fetch_annotation_owned_by(
    db: &DatabaseConnection,
    annotation_id: Uuid,
    expected: Uuid,
) -> Result<annotation::Model, McpError> {
    let row = annotation::Entity::find_by_id(annotation_id)
        .one(db)
        .await
        .map_err(db_error)?
        .ok_or_else(|| McpError::resource_not_found("annotation not found", None))?;
    if row.strategy_id != expected {
        return Err(invalid_params(format!(
            "forbidden: annotation {annotation_id} belongs to another strategy"
        )));
    }
    Ok(row)
}

pub(super) async fn ensure_strategy_exists(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), McpError> {
    let exists = strategy::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(db_error)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(invalid_params(format!("strategy {id} not found")))
    }
}

pub(super) fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or_else(|| {
        tracing::warn!(value = %d, "decimal value out of f64 range; coerced to 0.0");
        0.0
    })
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
}
