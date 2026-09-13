//! 戦略実行 MCP server の tool 実装
//!
//! 各戦略の t-rader-agent から呼ばれる。接続コンテキストに `x-strategy-id`
//! HTTP ヘッダで自身の strategy_id を持ち込み、全 tool はこの値のみを戦略境界として
//! 使う (tool 引数に strategy_id は含まれない)。さらに対象リソース (note / annotation)
//! の strategy_id と一致するかを Repository 層で二重検査する。
//! 例外が 3 つある。`read_portfolio` は、戦略は口座内のお金の区分に過ぎず分析は口座全体を
//! 見る、という設計上ヘッダの値を口座全体の集計にはスコープとして使わないが、
//! 接続元戦略自身のスライスを追加で返すためにヘッダの値も使う。`search_refs` は
//! stock/indicator/sector/theme が戦略に属さないマスタデータであるため、
//! ヘッダの値をそもそも検索条件に使わない。`search_news` も同様に news_item 全体を対象に
//! 検索するため、ヘッダの値を検索条件に使わない。
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
//! - `list_watch_targets`: 人間が `origin=human` で登録した監視対象銘柄
//!   (`ref_kind=stock`, `status=active`) を一覧する。保有状況によるフィルタは行わない
//! - `eval_indicator`: DB の indicator (戦略 scope 優先、無ければ global) を exec Pod 上で評価する
//! - `query_media`: 動画/音声 URL (YouTube 等) の内容を Gemini でテキスト化する
//! - `search_web`: 問い合わせ文で web 検索し、テキストと出典 URL を返す。モデルは既定で
//!   ChatGPT Plus 経由、`WEB_SEARCH_MODEL` で上書き可。戦略タスク実行単位で呼び出し回数に上限あり
//! - `read_portfolio`: 口座全体 (全戦略横断) の保有銘柄と実現損益に加え、接続元戦略自身の
//!   スライスを時価で返す
//! - `check_buyable_qty`: 指定銘柄をあと何株買えるかを、セクター上限比率・現金の各制約ごとに
//!   計算して返す
//! - `read_news`: 戦略に紐づく未読ニュースを checkpoint 以降分だけ古い順に返す
//! - `search_news`: news_item をキーワード / 期間で直接検索する (news_strategy_link 非経由)
//! - `search_refs`: 参照型 (stock/indicator/sector/theme) を id/name の部分一致で横断検索する
//! - `list_hypotheses`: 接続元戦略の仮説 + account-wide (global) 仮説を一覧する
//! - `read_hypothesis`: 単一の仮説を読む (自戦略または global)
//! - `propose_hypothesis_change`: 仮説へのタイトル/本文/status の変更を提案として永続化する
//!   (仮説本体には反映しない。人間が API 側で承認するまで適用されない)
//!
//! 実装はドメインごとに分割している:
//!
//! - `dto`: 各 tool の入出力スキーマ
//! - `notes`: ノート操作 (`write_note_inner` / `read_note_inner` / `list_notes_inner`)
//! - `annotations`: アノテーション操作 (`create_annotation_inner` / `read_annotations_inner`)
//! - `comments`: コメント操作 (`read_comments_inner` / `resolve_comment_inner` / `reply_comment_inner`)
//! - `data`: 価格データ取得 (`query_data_inner`)
//! - `evidence`: 外部データ取得の証跡記録 (`record_query_data`)
//! - `eval`: Python 実行 (`eval_python_inner`)
//! - `interests`: 関心の追加 (`add_interest_inner`) / 監視対象一覧 (`list_watch_targets_inner`)
//! - `eval_indicator`: 永続化された indicator の評価 (`eval_indicator_inner`)
//! - `hypotheses`: 仮説の読み取り / 変更提案 (`list_hypotheses_inner` / `read_hypothesis_inner` /
//!   `propose_hypothesis_change_inner`)
//! - `media`: 動画/音声 URL の Gemini によるテキスト化 (`query_media_inner`)
//! - `web_search`: 問い合わせ文の web 検索、テキストと出典 URL の返却 (`search_web_inner`)
//! - `news`: checkpoint を進めながら未読ニュースを返す (`read_news_inner`) /
//!   news_item のキーワード・期間検索 (`search_news_inner`)
//! - `portfolio`: 口座全体のポートフォリオ集計 (`read_portfolio_inner`)
//! - `risk_check`: 銘柄の追加購入可能株数の算出 (`check_buyable_qty_inner`)
//! - `refs`: 参照型 (stock/indicator/sector/theme) の横断検索 (`search_refs_inner`)
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
pub(super) mod evidence;
pub(super) mod hypotheses;
pub(super) mod interests;
pub(super) mod media;
pub(super) mod news;
pub(super) mod notes;
pub(super) mod portfolio;
pub(super) mod refs;
pub(super) mod risk_check;
mod tool_router;
pub(super) mod web_search;

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
/// `x-execution-id` ヘッダ名。agent は `{a2a_task_id}:{step_id}` 形式の値を MCP tool 呼び出し
/// ごとに送る (`step_id` は agent 内の実行ステップ 1 件を指す不透明な文字列)。backend は
/// これを `note.execution_id` にそのまま保持するが FK/join は持たず、単なる相関用の
/// 不透明な文字列として扱う。
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

/// `x-execution-id` ヘッダ値 (`{a2a_task_id}:{step_id}`) から `step_id` を取り出す。
/// FK を持たない理由は `backend/src/mcp/strategy/evidence.rs` を参照。
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
    if row.strategy_id != Some(expected) {
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
    if row.strategy_id != Some(expected) {
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

/// `AppError` の MCP エラー変換。`services::investable_amount` / `services::account_risk_policy`
/// / `models::risk_policy::parse_risk_policy` が返すエラーの共通ハンドリング。
pub(super) fn app_error_to_mcp(err: crate::error::AppError) -> McpError {
    use crate::error::AppError;
    match err {
        AppError::Database(e) => db_error(e),
        other => internal_error(format!("{other}")),
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
