//! 戦略実行 MCP の各 tool が交換する入出力スキーマ。
//!
//! ここでは型定義のみを置く。
//! ビジネスロジックは `notes` / `annotations` / `data` 配下に分かれている。

use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    /// `null` を明示すると既存タグを NULL に更新する。フィールド省略時は変更しない。
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub type_tag: Option<Option<String>>,
    pub frontmatter_json: Option<serde_json::Value>,
}

/// `Option<Option<T>>` を「フィールド未指定 (None)」と「null 指定 (Some(None))」に区別して受け取る
pub(super) fn deserialize_optional_field<'de, T, D>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalPythonParams {
    pub strategy_id: Uuid,
    /// 実行する Python コード本体 (utf-8)
    pub code: String,
    /// 実行中に Python の sys.stdin に流す入力
    pub stdin: Option<String>,
    /// wall-clock 上限。MCP 層の上限値を超える指定は invalid_params で拒否する。
    pub timeout_secs: Option<u32>,
    /// stdout + stderr の合計バイト数の上限。MCP 層の上限値を超える指定は
    /// invalid_params で拒否する。
    pub max_output_bytes: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct EvalPythonResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
