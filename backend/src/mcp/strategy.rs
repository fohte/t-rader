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

use std::sync::Arc;

use chrono::{DateTime, FixedOffset, NaiveDate};
use rmcp::ErrorData as McpError;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use schemars::JsonSchema;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data_provider::{DataProvider, DataProviderError, DataProviderKind, DateRange};
use crate::entities::{annotation, note, strategy};

const DEFAULT_LIST_LIMIT: u64 = 50;
const MAX_LIST_LIMIT: u64 = 200;

/// 戦略 Agent からの書き込み時に記録する actor 種別。
/// DB の CHECK 制約で `"human"` / `"llm"` のみ許容されているため `"llm"` を用いる。
const STRATEGY_AGENT_ACTOR: &str = "llm";

const ALLOWED_ANNOTATION_KINDS: [&str; 4] = ["signal", "level", "observation", "other"];
const DEFAULT_NOTE_STATUS: &str = "unread";
const DEFAULT_ANNOTATION_STATUS: &str = "unread";
const STRATEGY_ID_HEADER: &str = "x-strategy-id";

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

// === tool 入出力のスキーマ ===

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryDataParams {
    pub strategy_id: Uuid,
    pub instrument_id: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: NaiveDate,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: NaiveDate,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct BarDto {
    pub timestamp: DateTime<FixedOffset>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryDataResult {
    pub instrument_id: String,
    pub bars: Vec<BarDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteNoteParams {
    pub strategy_id: Uuid,
    /// 与えられたら既存ノートを更新する。省略時は新規作成する。
    pub note_id: Option<Uuid>,
    pub title: Option<String>,
    pub body_md: Option<String>,
    pub type_tag: Option<String>,
    pub frontmatter_json: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct WriteNoteResult {
    pub note_id: Uuid,
    pub created: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadNoteParams {
    pub strategy_id: Uuid,
    pub note_id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct NoteDto {
    pub note_id: Uuid,
    pub strategy_id: Uuid,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: serde_json::Value,
    pub type_tag: Option<String>,
    pub status: String,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    pub strategy_id: Uuid,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ListNotesResult {
    pub notes: Vec<NoteDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateAnnotationParams {
    pub strategy_id: Uuid,
    pub target_symbol: String,
    /// "signal" | "level" | "observation" | "other"
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<f64>,
    pub text: String,
    pub linked_note_id: Option<Uuid>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct AnnotationDto {
    pub annotation_id: Uuid,
    pub strategy_id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<f64>,
    pub text: String,
    pub status: String,
    pub linked_note_id: Option<Uuid>,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CreateAnnotationResult {
    pub annotation: AnnotationDto,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadAnnotationsParams {
    pub strategy_id: Uuid,
    pub target_symbol: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadAnnotationsResult {
    pub annotations: Vec<AnnotationDto>,
}

// === ヘルパー / エラーマッピング ===

fn internal_error(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::internal_error(msg, None)
}

fn invalid_params(msg: impl Into<std::borrow::Cow<'static, str>>) -> McpError {
    McpError::invalid_params(msg, None)
}

fn db_error(err: sea_orm::DbErr) -> McpError {
    tracing::error!(error = %err, "strategy mcp db error");
    internal_error(format!("database error: {err}"))
}

fn data_provider_error(err: DataProviderError) -> McpError {
    tracing::warn!(error = %err, "strategy mcp data provider error");
    match err {
        DataProviderError::NotFound(msg) => {
            McpError::resource_not_found(format!("instrument not found: {msg}"), None)
        }
        other => internal_error(format!("data provider error: {other}")),
    }
}

fn clamp_limit(limit: Option<u32>) -> u64 {
    let value = limit.map(u64::from).unwrap_or(DEFAULT_LIST_LIMIT);
    value.clamp(1, MAX_LIST_LIMIT)
}

/// `x-strategy-id` HTTP ヘッダを `HeaderMap` から取り出して Uuid に解釈する。
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

/// 接続コンテキストから `x-strategy-id` ヘッダを取り出して Uuid に解釈する。
fn strategy_id_from_ctx(ctx: &RequestContext<RoleServer>) -> Result<Uuid, McpError> {
    let parts = ctx
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| internal_error("missing http parts in mcp request context"))?;
    strategy_id_from_headers(&parts.headers)
}

/// 接続コンテキストの strategy_id と引数の strategy_id が一致することを検査する。
/// 不一致は MCP エラー (forbidden) で拒否する。
fn ensure_strategy_match(ctx_id: Uuid, arg_id: Uuid) -> Result<(), McpError> {
    if ctx_id == arg_id {
        Ok(())
    } else {
        Err(invalid_params(format!(
            "forbidden: strategy_id boundary violation (session={ctx_id}, arg={arg_id})"
        )))
    }
}

/// note を取得し、所有戦略が `expected` でなければ境界違反として弾く。
async fn fetch_note_owned_by(
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

/// 戦略 id が strategy テーブルに存在することを確認する。
async fn ensure_strategy_exists(db: &DatabaseConnection, id: Uuid) -> Result<(), McpError> {
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

fn ensure_frontmatter_object(fm: &serde_json::Value) -> Result<(), McpError> {
    if fm.is_object() {
        Ok(())
    } else {
        Err(invalid_params("frontmatter_json must be a JSON object"))
    }
}

fn note_to_dto(m: note::Model) -> NoteDto {
    NoteDto {
        note_id: m.id,
        strategy_id: m.strategy_id,
        title: m.title,
        body_md: m.body_md,
        frontmatter_json: m.frontmatter_json,
        type_tag: m.type_tag,
        status: m.status,
        created_by_kind: m.created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

fn annotation_to_dto(m: annotation::Model) -> AnnotationDto {
    AnnotationDto {
        annotation_id: m.id,
        strategy_id: m.strategy_id,
        target_symbol: m.target_symbol,
        target_kind: m.target_kind,
        timestamp: m.timestamp,
        price: m.price.map(decimal_to_f64),
        text: m.text,
        status: m.status,
        linked_note_id: m.linked_note_id,
        created_by_kind: m.created_by_kind,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

/// `Decimal` を JSON 用に `f64` へ落とす。範囲外で `to_f64` が `None` を返した場合は警告ログを残して 0.0 にフォールバックする。
fn decimal_to_f64(d: Decimal) -> f64 {
    d.to_f64().unwrap_or_else(|| {
        tracing::warn!(value = %d, "decimal value out of f64 range; coerced to 0.0");
        0.0
    })
}

fn f64_to_decimal(v: f64) -> Result<Decimal, McpError> {
    Decimal::try_from(v).map_err(|err| invalid_params(format!("invalid decimal value: {err}")))
}

impl StrategyServer {
    pub(crate) async fn query_data_inner(
        &self,
        session_strategy_id: Uuid,
        params: QueryDataParams,
    ) -> Result<QueryDataResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let instrument_id = params.instrument_id.trim().to_string();
        if instrument_id.is_empty() {
            return Err(invalid_params("instrument_id must not be empty"));
        }
        if params.from > params.to {
            return Err(invalid_params("from must be on or before to"));
        }

        let provider = self
            .data_provider
            .as_deref()
            .ok_or_else(|| internal_error("data provider is not configured"))?;

        let bars = provider
            .fetch_daily_bars(
                &instrument_id,
                &DateRange {
                    from: params.from,
                    to: params.to,
                },
            )
            .await
            .map_err(data_provider_error)?;

        let bars = bars
            .into_iter()
            .map(|b| BarDto {
                timestamp: b.timestamp.fixed_offset(),
                open: decimal_to_f64(b.open),
                high: decimal_to_f64(b.high),
                low: decimal_to_f64(b.low),
                close: decimal_to_f64(b.close),
                volume: b.volume,
            })
            .collect();
        Ok(QueryDataResult {
            instrument_id,
            bars,
        })
    }

    pub(crate) async fn write_note_inner(
        &self,
        session_strategy_id: Uuid,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        if let Some(fm) = params.frontmatter_json.as_ref() {
            ensure_frontmatter_object(fm)?;
        }

        if let Some(note_id) = params.note_id {
            let current = fetch_note_owned_by(&self.db, note_id, params.strategy_id).await?;
            let mut active = current.clone().into_active_model();
            let mut touched = false;
            if let Some(title) = params.title {
                let title = title.trim().to_string();
                if title.is_empty() {
                    return Err(invalid_params("title must not be empty"));
                }
                active.title = Set(title);
                touched = true;
            }
            if let Some(body) = params.body_md {
                active.body_md = Set(body);
                touched = true;
            }
            if let Some(tag) = params.type_tag {
                active.type_tag = Set(Some(tag));
                touched = true;
            }
            if let Some(fm) = params.frontmatter_json {
                active.frontmatter_json = Set(fm);
                touched = true;
            }
            if !touched {
                return Err(invalid_params(
                    "at least one of title / body_md / type_tag / frontmatter_json must be provided",
                ));
            }
            active.updated_at = Set(chrono::Utc::now().fixed_offset());
            active.update(&self.db).await.map_err(db_error)?;
            return Ok(WriteNoteResult {
                note_id,
                created: false,
            });
        }

        // create
        ensure_strategy_exists(&self.db, params.strategy_id).await?;
        let title = params
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid_params("title is required when creating a new note"))?
            .to_string();
        let body_md = params.body_md.unwrap_or_default();
        let frontmatter_json = params
            .frontmatter_json
            .unwrap_or_else(|| serde_json::json!({}));
        let id = Uuid::new_v4();
        let model = note::ActiveModel {
            id: Set(id),
            strategy_id: Set(params.strategy_id),
            title: Set(title),
            body_md: Set(body_md),
            frontmatter_json: Set(frontmatter_json),
            type_tag: Set(params.type_tag),
            status: Set(DEFAULT_NOTE_STATUS.to_string()),
            trigger: Set(None),
            trigger_label: Set(None),
            created_by_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
        };
        note::Entity::insert(model)
            .exec_without_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(WriteNoteResult {
            note_id: id,
            created: true,
        })
    }

    pub(crate) async fn read_note_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadNoteParams,
    ) -> Result<NoteDto, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let row = fetch_note_owned_by(&self.db, params.note_id, params.strategy_id).await?;
        Ok(note_to_dto(row))
    }

    pub(crate) async fn list_notes_inner(
        &self,
        session_strategy_id: Uuid,
        params: ListNotesParams,
    ) -> Result<ListNotesResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let rows = note::Entity::find()
            .filter(note::Column::StrategyId.eq(params.strategy_id))
            .order_by_desc(note::Column::UpdatedAt)
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ListNotesResult {
            notes: rows.into_iter().map(note_to_dto).collect(),
        })
    }

    pub(crate) async fn create_annotation_inner(
        &self,
        session_strategy_id: Uuid,
        params: CreateAnnotationParams,
    ) -> Result<CreateAnnotationResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let target_symbol = params.target_symbol.trim().to_string();
        if target_symbol.is_empty() {
            return Err(invalid_params("target_symbol must not be empty"));
        }
        if !ALLOWED_ANNOTATION_KINDS.contains(&params.target_kind.as_str()) {
            return Err(invalid_params(format!(
                "invalid target_kind: {} (expected one of {:?})",
                params.target_kind, ALLOWED_ANNOTATION_KINDS
            )));
        }
        if params.text.trim().is_empty() {
            return Err(invalid_params("text must not be empty"));
        }

        ensure_strategy_exists(&self.db, params.strategy_id).await?;

        // linked_note_id が指定されている場合、対象 note の strategy_id 一致を二重検査する
        if let Some(linked) = params.linked_note_id {
            fetch_note_owned_by(&self.db, linked, params.strategy_id).await?;
        }

        let price = params.price.map(f64_to_decimal).transpose()?;
        let id = Uuid::new_v4();
        let model = annotation::ActiveModel {
            id: Set(id),
            strategy_id: Set(params.strategy_id),
            target_symbol: Set(target_symbol),
            target_kind: Set(params.target_kind),
            timestamp: Set(params.timestamp),
            price: Set(price),
            text: Set(params.text),
            status: Set(DEFAULT_ANNOTATION_STATUS.to_string()),
            linked_note_id: Set(params.linked_note_id),
            created_by_kind: Set(STRATEGY_AGENT_ACTOR.to_string()),
            created_at: NotSet,
            updated_at: NotSet,
        };
        let created = annotation::Entity::insert(model)
            .exec_with_returning(&self.db)
            .await
            .map_err(db_error)?;
        Ok(CreateAnnotationResult {
            annotation: annotation_to_dto(created),
        })
    }

    pub(crate) async fn read_annotations_inner(
        &self,
        session_strategy_id: Uuid,
        params: ReadAnnotationsParams,
    ) -> Result<ReadAnnotationsResult, McpError> {
        ensure_strategy_match(session_strategy_id, params.strategy_id)?;

        let mut q = annotation::Entity::find()
            .filter(annotation::Column::StrategyId.eq(params.strategy_id))
            .order_by_desc(annotation::Column::Timestamp);
        if let Some(sym) = params
            .target_symbol
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            q = q.filter(annotation::Column::TargetSymbol.eq(sym));
        }
        let rows = q
            .limit(clamp_limit(params.limit))
            .all(&self.db)
            .await
            .map_err(db_error)?;
        Ok(ReadAnnotationsResult {
            annotations: rows.into_iter().map(annotation_to_dto).collect(),
        })
    }
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
        let err = ensure_strategy_match(a, b).err().unwrap_or_else(|| {
            panic!("expected mismatch error");
        });
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

#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;

    use sea_orm::ActiveModelTrait;
    use sqlx::PgPool;

    use crate::data_provider::ibkr::mock::{IbkrMockServer, MockHistoryBar};
    use crate::testing::create_test_db;

    use super::*;

    async fn insert_strategy(db: &DatabaseConnection, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        strategy::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
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

    fn build_server(db: DatabaseConnection) -> StrategyServer {
        StrategyServer::new(db, None)
    }

    /// テスト用 sentinel: DTO の比較で動的な timestamp を差し替える
    fn ts_sentinel() -> DateTime<FixedOffset> {
        chrono::DateTime::<chrono::Utc>::UNIX_EPOCH.fixed_offset()
    }

    /// 動的な `created_at` / `updated_at` を sentinel に置き換える
    fn normalize_note(mut n: NoteDto) -> NoteDto {
        n.created_at = ts_sentinel();
        n.updated_at = ts_sentinel();
        n
    }

    /// 動的な id / timestamp を sentinel に置き換える
    fn normalize_annotation(mut a: AnnotationDto) -> AnnotationDto {
        a.created_at = ts_sentinel();
        a.updated_at = ts_sentinel();
        a
    }

    /// 戦略 B の所有として固定タイトルの note を seed する
    async fn seed_foreign_note(db: &DatabaseConnection, owner: Uuid, title: &str) -> Uuid {
        let id = Uuid::new_v4();
        note::ActiveModel {
            id: Set(id),
            strategy_id: Set(owner),
            title: Set(title.to_string()),
            body_md: Set("body".into()),
            frontmatter_json: Set(serde_json::json!({})),
            type_tag: Set(None),
            status: Set(DEFAULT_NOTE_STATUS.into()),
            trigger: Set(None),
            trigger_label: Set(None),
            created_by_kind: Set("llm".into()),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("seed note");
        id
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_creates_then_read_note_returns_it(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        let server = build_server(db);

        let written = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: None,
                    title: Some("first note".into()),
                    body_md: Some("body".into()),
                    type_tag: Some("observation".into()),
                    frontmatter_json: None,
                },
            )
            .await
            .expect("write_note");
        assert!(written.created);

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    strategy_id,
                    note_id: written.note_id,
                },
            )
            .await
            .expect("read_note");

        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: written.note_id,
                strategy_id,
                title: "first note".into(),
                body_md: "body".into(),
                frontmatter_json: serde_json::json!({}),
                type_tag: Some("observation".into()),
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_updates_existing(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);

        let created = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: None,
                    title: Some("original".into()),
                    body_md: Some("v1".into()),
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect("create");

        let updated = server
            .write_note_inner(
                strategy_id,
                WriteNoteParams {
                    strategy_id,
                    note_id: Some(created.note_id),
                    title: None,
                    body_md: Some("v2".into()),
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect("update");
        assert_eq!(
            updated,
            WriteNoteResult {
                note_id: created.note_id,
                created: false,
            },
        );

        let read = server
            .read_note_inner(
                strategy_id,
                ReadNoteParams {
                    strategy_id,
                    note_id: created.note_id,
                },
            )
            .await
            .expect("read");
        assert_eq!(
            normalize_note(read),
            NoteDto {
                note_id: created.note_id,
                strategy_id,
                title: "original".into(),
                body_md: "v2".into(),
                frontmatter_json: serde_json::json!({}),
                type_tag: None,
                status: DEFAULT_NOTE_STATUS.into(),
                created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                created_at: ts_sentinel(),
                updated_at: ts_sentinel(),
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_arg_mismatch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db);

        // session の strategy_id は A、引数は B → boundary violation
        let err = server
            .write_note_inner(
                strategy_a,
                WriteNoteParams {
                    strategy_id: strategy_b,
                    note_id: None,
                    title: Some("x".into()),
                    body_md: None,
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect_err("boundary violation expected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn write_note_rejects_cross_strategy_update(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        // 戦略 A の Agent (session + 引数とも A) が B の note を更新しようとする → Repository 検査で拒否
        let err = server
            .write_note_inner(
                strategy_a,
                WriteNoteParams {
                    strategy_id: strategy_a,
                    note_id: Some(note_id),
                    title: None,
                    body_md: Some("hijack".into()),
                    type_tag: None,
                    frontmatter_json: None,
                },
            )
            .await
            .expect_err("cross-strategy update expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn read_note_rejects_cross_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let note_id = seed_foreign_note(&db, strategy_b, "b's note").await;

        // 戦略 A の引数で B のノートを読もうとする → 引数が A、対象が B で Repository 検査により拒否
        let err = server
            .read_note_inner(
                strategy_a,
                ReadNoteParams {
                    strategy_id: strategy_a,
                    note_id,
                },
            )
            .await
            .expect_err("cross-strategy read expected to fail");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn list_notes_filters_by_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db);

        for (sid, title) in [(strategy_a, "a1"), (strategy_a, "a2"), (strategy_b, "b1")] {
            server
                .write_note_inner(
                    sid,
                    WriteNoteParams {
                        strategy_id: sid,
                        note_id: None,
                        title: Some(title.into()),
                        body_md: None,
                        type_tag: None,
                        frontmatter_json: None,
                    },
                )
                .await
                .expect("write");
        }

        let result = server
            .list_notes_inner(
                strategy_a,
                ListNotesParams {
                    strategy_id: strategy_a,
                    limit: None,
                },
            )
            .await
            .expect("list");
        // 戦略 B のノートは含まれず、戦略 A の 2 件のみが新しい順に並ぶ
        let titles: Vec<&str> = result.notes.iter().map(|n| n.title.as_str()).collect();
        let strategies: Vec<Uuid> = result.notes.iter().map(|n| n.strategy_id).collect();
        assert_eq!(
            (titles, strategies),
            (vec!["a2", "a1"], vec![strategy_a, strategy_a]),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_annotation_then_read_annotations(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "swing").await;
        let server = build_server(db);
        let ts: DateTime<FixedOffset> = "2026-06-01T09:00:00+09:00".parse().expect("ts");

        let created = server
            .create_annotation_inner(
                strategy_id,
                CreateAnnotationParams {
                    strategy_id,
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: ts,
                    price: Some(25000.0),
                    text: "breakout".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect("create");
        let expected = AnnotationDto {
            annotation_id: created.annotation.annotation_id,
            strategy_id,
            target_symbol: "7203".into(),
            target_kind: "signal".into(),
            timestamp: ts,
            price: Some(25000.0),
            text: "breakout".into(),
            status: DEFAULT_ANNOTATION_STATUS.into(),
            linked_note_id: None,
            created_by_kind: STRATEGY_AGENT_ACTOR.into(),
            created_at: ts_sentinel(),
            updated_at: ts_sentinel(),
        };
        assert_eq!(normalize_annotation(created.annotation), expected,);

        let list = server
            .read_annotations_inner(
                strategy_id,
                ReadAnnotationsParams {
                    strategy_id,
                    target_symbol: None,
                    limit: None,
                },
            )
            .await
            .expect("list");
        assert_eq!(
            ReadAnnotationsResult {
                annotations: list
                    .annotations
                    .into_iter()
                    .map(normalize_annotation)
                    .collect(),
            },
            ReadAnnotationsResult {
                annotations: vec![expected],
            },
        );
    }

    #[sqlx::test(migrations = false)]
    async fn create_annotation_rejects_invalid_kind(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db);
        let err = server
            .create_annotation_inner(
                strategy_id,
                CreateAnnotationParams {
                    strategy_id,
                    target_symbol: "7203".into(),
                    target_kind: "garbage".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "x".into(),
                    linked_note_id: None,
                },
            )
            .await
            .expect_err("invalid kind expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn create_annotation_rejects_cross_strategy_linked_note(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_a = insert_strategy(&db, "a").await;
        let strategy_b = insert_strategy(&db, "b").await;
        let server = build_server(db.clone());
        let foreign_note = seed_foreign_note(&db, strategy_b, "b").await;

        let err = server
            .create_annotation_inner(
                strategy_a,
                CreateAnnotationParams {
                    strategy_id: strategy_a,
                    target_symbol: "7203".into(),
                    target_kind: "signal".into(),
                    timestamp: "2026-06-01T00:00:00Z".parse().expect("ts"),
                    price: None,
                    text: "x".into(),
                    linked_note_id: Some(foreign_note),
                },
            )
            .await
            .expect_err("cross-strategy linked note expected to be rejected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn query_data_returns_bars_from_mock_ibkr(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;

        let ibkr = IbkrMockServer::start().await;
        ibkr.stocks().ok().await;
        ibkr.history()
            .bars(vec![
                MockHistoryBar {
                    t: 1_736_121_600_000,
                    o: 100.0,
                    h: 110.0,
                    l: 90.0,
                    c: 105.0,
                    v: 1_000.0,
                },
                MockHistoryBar {
                    t: 1_736_208_000_000,
                    o: 105.0,
                    h: 115.0,
                    l: 95.0,
                    c: 110.0,
                    v: 1_500.0,
                },
            ])
            .ok()
            .await;
        let client = ibkr.client().expect("client");
        let provider = Arc::new(DataProviderKind::Ibkr(client));

        let server = StrategyServer::new(db, Some(provider));

        let result = server
            .query_data_inner(
                strategy_id,
                QueryDataParams {
                    strategy_id,
                    instrument_id: "7203".into(),
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect("query");
        let bars: Vec<(f64, f64, f64, f64, i64)> = result
            .bars
            .iter()
            .map(|b| (b.open, b.high, b.low, b.close, b.volume))
            .collect();
        assert_eq!(
            (result.instrument_id.as_str(), bars),
            (
                "7203",
                vec![
                    (100.0, 110.0, 90.0, 105.0, 1_000),
                    (105.0, 115.0, 95.0, 110.0, 1_500),
                ],
            ),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn query_data_rejects_session_mismatch(pool: PgPool) {
        let db = create_test_db(pool).await;
        let session = insert_strategy(&db, "a").await;
        let other = insert_strategy(&db, "b").await;
        let server = build_server(db);
        let err = server
            .query_data_inner(
                session,
                QueryDataParams {
                    strategy_id: other,
                    instrument_id: "7203".into(),
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect_err("boundary violation expected");
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }
}
