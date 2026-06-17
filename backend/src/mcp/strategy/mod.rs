//! 戦略実行 MCP server の tool 実装
//!
//! 各戦略の kubeopencode Agent から呼ばれる。接続コンテキストに `x-strategy-id`
//! HTTP ヘッダで自身の strategy_id を持ち込み、tool 引数の strategy_id と一致しない
//! 呼び出しは MCP 層で拒否する (戦略境界)。さらに対象リソース (note / annotation)
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
//!
//! 実装はドメインごとに分割している:
//!
//! - `dto`: 各 tool の入出力スキーマ
//! - `notes`: ノート操作 (`write_note_inner` / `read_note_inner` / `list_notes_inner`)
//! - `annotations`: アノテーション操作 (`create_annotation_inner` / `read_annotations_inner`)
//! - `data`: 価格データ取得 (`query_data_inner`)
//!
//! 本モジュールは tool wrapper (`#[tool_router]` / `#[tool_handler]`) と
//! 戦略境界・エラー変換などドメイン横断のヘルパを担う。

pub(super) mod annotations;
pub(super) mod data;
pub(super) mod dto;
pub(super) mod notes;

#[cfg(test)]
mod tests_common;

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::data_provider::{DataProviderError, DataProviderKind};
use crate::entities::{note, strategy};

use dto::{
    CreateAnnotationParams, CreateAnnotationResult, ListNotesParams, ListNotesResult, NoteDto,
    QueryDataParams, QueryDataResult, ReadAnnotationsParams, ReadAnnotationsResult, ReadNoteParams,
    WriteNoteParams, WriteNoteResult,
};

const DEFAULT_LIST_LIMIT: u64 = 50;
const MAX_LIST_LIMIT: u64 = 200;

/// 戦略 Agent からの書き込み時に記録する actor 種別。
/// DB の CHECK 制約で `"human"` / `"llm"` のみ許容されているため `"llm"` を用いる。
pub(super) const STRATEGY_AGENT_ACTOR: &str = "llm";

const STRATEGY_ID_HEADER: &str = "x-strategy-id";

pub(super) const DEFAULT_NOTE_STATUS: &str = "unread";
pub(super) const DEFAULT_ANNOTATION_STATUS: &str = "unread";
pub(super) const ALLOWED_ANNOTATION_KINDS: [&str; 4] = ["signal", "level", "observation", "other"];

#[derive(Clone)]
pub struct StrategyServer {
    db: DatabaseConnection,
    data_provider: Option<Arc<DataProviderKind>>,
}

impl StrategyServer {
    pub fn new(db: DatabaseConnection, data_provider: Option<Arc<DataProviderKind>>) -> Self {
        Self { db, data_provider }
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

pub(super) fn ensure_strategy_match(ctx_id: Uuid, arg_id: Uuid) -> Result<(), McpError> {
    if ctx_id == arg_id {
        Ok(())
    } else {
        Err(invalid_params(format!(
            "forbidden: strategy_id boundary violation (session={ctx_id}, arg={arg_id})"
        )))
    }
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

#[tool_router]
impl StrategyServer {
    /// 銘柄 + 期間で日足バーデータを取得する
    #[tool(
        name = "query_data",
        description = "Fetch daily OHLCV bars for an instrument over a date range via the configured data provider."
    )]
    async fn query_data(
        &self,
        Parameters(params): Parameters<QueryDataParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<QueryDataResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.query_data_inner(sid, params).await.map(Json)
    }

    /// ノートを作成または更新する
    #[tool(
        name = "write_note",
        description = "Create a new note or update an existing note owned by the strategy. Supply note_id to update; omit it to create."
    )]
    async fn write_note(
        &self,
        Parameters(params): Parameters<WriteNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<WriteNoteResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.write_note_inner(sid, params).await.map(Json)
    }

    /// ノートを読み出す
    #[tool(
        name = "read_note",
        description = "Read a single note owned by the strategy."
    )]
    async fn read_note(
        &self,
        Parameters(params): Parameters<ReadNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<NoteDto>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_note_inner(sid, params).await.map(Json)
    }

    /// 戦略のノート一覧を返す (新しい順)
    #[tool(
        name = "list_notes",
        description = "List notes owned by the strategy, newest first."
    )]
    async fn list_notes(
        &self,
        Parameters(params): Parameters<ListNotesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ListNotesResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.list_notes_inner(sid, params).await.map(Json)
    }

    /// アノテーションを作成する
    #[tool(
        name = "create_annotation",
        description = "Create a chart annotation owned by the strategy."
    )]
    async fn create_annotation(
        &self,
        Parameters(params): Parameters<CreateAnnotationParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<CreateAnnotationResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.create_annotation_inner(sid, params).await.map(Json)
    }

    /// 戦略のアノテーション一覧を返す
    #[tool(
        name = "read_annotations",
        description = "List annotations owned by the strategy. Optionally filter by target_symbol."
    )]
    async fn read_annotations(
        &self,
        Parameters(params): Parameters<ReadAnnotationsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<Json<ReadAnnotationsResult>, McpError> {
        let sid = strategy_id_from_ctx(&ctx)?;
        self.read_annotations_inner(sid, params).await.map(Json)
    }
}

#[tool_handler]
impl ServerHandler for StrategyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("t-rader-strategy", env!("CARGO_PKG_VERSION")),
        )
    }
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

    #[test]
    fn ensure_strategy_match_ok() {
        let id = Uuid::new_v4();
        assert!(ensure_strategy_match(id, id).is_ok());
    }

    #[test]
    fn ensure_strategy_match_rejects_mismatch() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let err =
            ensure_strategy_match(a, b).expect_err("mismatched strategy ids expected to error");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    fn headers_with(strategy: Option<&str>) -> axum::http::HeaderMap {
        let mut h = axum::http::HeaderMap::new();
        if let Some(v) = strategy {
            h.insert(
                STRATEGY_ID_HEADER,
                axum::http::HeaderValue::from_str(v).expect("header value"),
            );
        }
        h
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
}
