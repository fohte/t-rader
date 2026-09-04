//! 戦略実行 MCP の各 tool が交換する入出力スキーマ。
//!
//! ここでは型定義のみを置く。
//! ビジネスロジックは `notes` / `annotations` / `data` 配下に分かれている。

use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::graph::GraphDef;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryDataParams {
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

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WriteNoteParams {
    /// 与えられたら既存ノートを更新する。省略時は新規作成する。
    pub note_id: Option<Uuid>,
    pub title: Option<String>,
    pub body_md: Option<String>,
    /// `null` を明示すると既存タグを NULL に更新する。フィールド省略時は変更しない。
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub type_tag: Option<Option<String>>,
    pub frontmatter_json: Option<serde_json::Map<String, serde_json::Value>>,
    /// ノートに埋め込む図の定義。指定すると既存の図を配列ごと置き換える
    /// (id 単位の部分更新はできない)。省略時は既存の図を変更しない。
    /// 各要素の `id` を本文中で `[[graph:<id>]]` として参照すること。
    /// `value` (ノード/エッジのサイズ) を指定する場合は出典を示す `cite` も必須。
    pub graphs: Option<Vec<GraphDef>>,
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
    pub note_id: Uuid,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct NoteDto {
    pub note_id: Uuid,
    pub strategy_id: Uuid,
    pub title: String,
    pub body_md: String,
    pub frontmatter_json: serde_json::Map<String, serde_json::Value>,
    pub type_tag: Option<String>,
    pub status: String,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub graphs: Vec<GraphDef>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListNotesResult {
    pub notes: Vec<NoteDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateAnnotationParams {
    pub target_symbol: String,
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
    pub target_symbol: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadAnnotationsResult {
    pub annotations: Vec<AnnotationDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddInterestParams {
    /// 参照型 (`stock` / `indicator` / `sector` / `theme`)
    pub ref_kind: String,
    pub ref_id: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct AddInterestResult {
    pub strategy_id: Uuid,
    pub ref_kind: String,
    pub ref_id: String,
    pub role: String,
    pub origin: String,
    /// 既存と一致したため idempotent に成功した場合は false
    pub created: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadCommentsParams {
    /// "note" | "annotation"
    pub target_kind: String,
    pub target_id: Uuid,
    /// true/false で絞り込み。省略時は全件
    pub resolved: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct CommentDto {
    pub comment_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    /// 返信先コメント。スレッドの起点なら null。
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    pub resolved: bool,
    pub created_at: DateTime<FixedOffset>,
    /// コメント時点で選択された本文の該当箇所全文。
    pub anchor_text: Option<String>,
    /// note 本文中の現在位置 (1-indexed)。追跡できない場合は null。
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    /// 位置が当てにならなくなったかどうか (note 本文の書き換えで見失った等)。
    pub drifted: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ReadCommentsResult {
    pub comments: Vec<CommentDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveCommentParams {
    pub comment_id: Uuid,
    pub resolved: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ResolveCommentResult {
    pub comment: CommentDto,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyCommentParams {
    /// 返信先コメント ID
    pub parent_id: Uuid,
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ReplyCommentResult {
    pub comment: CommentDto,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalPythonParams {
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvalIndicatorParams {
    /// 評価する indicator の name。戦略 scope に同名があれば優先、無ければ global を採用する。
    pub name: String,
    /// indicator の `input_schema` (JSON Schema) で validation される引数オブジェクト。
    #[schemars(schema_with = "crate::mcp::any_json_schema")]
    pub args: serde_json::Value,
    /// wall-clock 上限 (秒)。MCP 層の上限値を超える指定は invalid_params で拒否する。
    pub timeout_secs: Option<u32>,
    /// stdout + stderr の合計バイト数の上限。MCP 層の上限値を超える指定は
    /// invalid_params で拒否する。
    pub max_output_bytes: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryMediaParams {
    /// 動画/音声の URL。YouTube の公開動画 URL を推奨。他の公開 https:// URL も試行できるが、
    /// Gemini 側で取得できない場合はエラーになる。
    pub media_url: String,
    /// 動画/音声から何を読み取りたいかを指示するプロンプト
    pub prompt: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct QueryMediaResult {
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct EvalIndicatorResult {
    /// 評価された indicator の id。
    pub indicator_id: Uuid,
    /// 解決された scope (`global` / `strategy`)。
    pub scope: String,
    /// stdout 最終行を JSON parse し output_schema で validation 済みの値。
    /// exec Pod が exit_code != 0 で終わった場合は null (stderr / exit_code を見ること)。
    /// stdout 最終行が JSON として parse できない / output_schema に合致しない場合は
    /// MCP エラー (invalid_params) で失敗するため、本フィールドには到達しない。
    #[serde(default)]
    #[schemars(schema_with = "crate::mcp::any_json_schema")]
    pub output: Option<serde_json::Value>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
