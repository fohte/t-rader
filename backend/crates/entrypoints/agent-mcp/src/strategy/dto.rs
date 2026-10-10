//! 戦略実行 MCP の各 tool が交換する入出力スキーマ。
//!
//! ここでは型定義のみを置く。
//! ビジネスロジックは `notes` / `annotations` / `data` 配下に分かれている。

use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::graph_dto::GraphDef;
use super::serde_helpers::deserialize_nullable_option;

mod annotations;
mod financials;
mod list_movers;
mod news;
mod paper_trade;
mod portfolio;
mod query_data;
pub use annotations::*;
pub use financials::*;
pub use list_movers::*;
pub use news::*;
pub use paper_trade::*;
pub use portfolio::*;
pub use query_data::*;

fn any_json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    serde_json::Map::new().into()
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteNoteParams {
    /// 与えられたら既存ノートを更新する。省略時は新規作成する。
    pub note_id: Option<Uuid>,
    pub title: Option<String>,
    /// `[[note:<uuid>]]` はリンク元バージョンを作成した時点の現行バージョンに固定する。
    /// `@current` を付けると以降の現行バージョンに追従する。
    pub body_md: Option<String>,
    /// 新規作成時の種別。既存ノートの種別は変更できない。
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_nullable_option"
    )]
    pub kind: Option<Option<String>>,
    /// 承認必須種別の 2 件目以降で必須となる変更理由。
    pub change_reason: Option<String>,
    pub frontmatter_json: Option<serde_json::Map<String, serde_json::Value>>,
    /// ノートに埋め込む図の定義。指定すると既存の図を配列ごと置き換える
    /// (id 単位の部分更新はできない)。省略時は既存の図を変更しない。
    /// 各要素の `id` を本文中で `[[graph:<id>]]` として参照すること。
    /// 図トークンは空行で区切られたブロック内に単独で置くこと。
    /// `value` (ノード/エッジのサイズ) を指定する場合は出典を示す `cite` も必須。
    pub graphs: Option<Vec<GraphDef>>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct WriteNoteResult {
    pub note_id: Uuid,
    pub created: bool,
    /// 本文の価格候補、相対表現、ローソク足の本数に関する警告。
    /// 誤検出を含む案内であり、ノートの保存は拒否されません。
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadNoteParams {
    pub note_id: Uuid,
    /// 省略時は現行バージョンを読む。
    pub version_id: Option<Uuid>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct NoteLinkDto {
    /// 参照先ノート ID。
    pub to_note_id: Uuid,
    /// 固定したバージョン ID。null の場合は参照先ノートの現行バージョンに追従する。
    pub to_version_id: Option<Uuid>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct NoteDto {
    pub note_id: Uuid,
    /// 本文が属するバージョン ID。`read_comments` の `target_id` に使う。
    pub version_id: Uuid,
    /// ノート内のバージョン番号。
    pub version_no: i32,
    pub title: String,
    /// `list_notes` で `include_body: false` を指定したときのみ省略される (null)。
    /// `read_note` の結果では常に値を含む
    pub body_md: Option<String>,
    pub frontmatter_json: serde_json::Map<String, serde_json::Value>,
    pub tags: Vec<String>,
    pub kind: Option<String>,
    pub status: String,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub graphs: Vec<GraphDef>,
    /// `read_note` の結果でのみ含まれる、このバージョンから出るリンク。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<NoteLinkDto>>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct NoteKindDto {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ListNoteKindsResult {
    pub note_kinds: Vec<NoteKindDto>,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct ListNotesParams {
    pub limit: Option<u32>,
    /// 指定した note_kind のノートだけを返す
    pub kind: Option<String>,
    /// `kind:id` の形式で参照先にリンクしたノートだけを返す。stock を指定すると所属するすべての group へのリンクも含む。
    #[serde(rename = "ref")]
    pub r#ref: Option<String>,
    /// "approved" / "unread" / "rejected" のいずれかで絞り込む。省略時は全 status
    pub status: Option<String>,
    /// この時刻以降 (inclusive) に更新されたノートのみ返す。省略時は下限なし
    pub updated_after: Option<DateTime<FixedOffset>>,
    /// false を指定すると body_md を省略し、レスポンスサイズを抑える。省略時は true (本文を含む)
    pub include_body: Option<bool>,
    /// true を指定すると現行バージョンがないノートも返す。最新バージョンを使用する
    pub include_pending: Option<bool>,
    /// `frontmatter_json.tags` に完全一致するタグを持つノートだけを返す。
    pub tag: Option<String>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListNotesResult {
    pub notes: Vec<NoteDto>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadCommentsParams {
    /// "note_version" | "annotation"
    pub target_kind: String,
    pub target_id: Uuid,
    /// true/false で絞り込み。省略時は全件
    pub resolved: Option<bool>,
}

#[cfg_attr(test, derive(Clone, serde::Deserialize))]
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
    /// 行コメントが対応する本文側。`note_version` の場合のみ設定される。
    pub anchor_side: Option<String>,
    /// 対応する本文中の行位置 (1-indexed)。
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ReadCommentsResult {
    pub comments: Vec<CommentDto>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResolveCommentParams {
    pub comment_id: Uuid,
    pub resolved: bool,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ResolveCommentResult {
    pub comment: CommentDto,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReplyCommentParams {
    /// 返信先コメント ID
    pub parent_id: Uuid,
    pub body: String,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ReplyCommentResult {
    pub comment: CommentDto,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct EvalPythonResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EvalIndicatorParams {
    /// 評価する indicator の name。戦略 scope に同名があれば優先、無ければ global を採用する。
    pub name: String,
    /// indicator の `input_schema` (JSON Schema) で validation される引数オブジェクト。
    #[schemars(schema_with = "any_json_schema")]
    pub args: serde_json::Value,
    /// wall-clock 上限 (秒)。MCP 層の上限値を超える指定は invalid_params で拒否する。
    pub timeout_secs: Option<u32>,
    /// stdout + stderr の合計バイト数の上限。MCP 層の上限値を超える指定は
    /// invalid_params で拒否する。
    pub max_output_bytes: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryYoutubeParams {
    /// YouTube 動画の URL
    pub youtube_url: String,
    /// 動画について回答する質問。1 件以上指定する。
    pub questions: Vec<String>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct QueryYoutubeResult {
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SearchWebParams {
    /// 検索したい内容を表す自然文の問い合わせ
    pub query: String,
    /// 検索対象。省略時は general
    #[serde(default)]
    pub topic: Option<SearchWebTopic>,
    /// 検索対象の記事公開時期
    #[serde(default)]
    pub time_range: Option<SearchWebTimeRange>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchWebTopic {
    General,
    News,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchWebTimeRange {
    Day,
    Week,
    Month,
    Year,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchWebResult {
    pub results: Vec<SearchWebArticle>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct SearchWebArticle {
    pub title: String,
    pub url: String,
    pub published_date: Option<String>,
    pub snippet: String,
    pub body: Option<String>,
    pub body_truncated: bool,
}

#[cfg_attr(test, derive(serde::Deserialize))]
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
    #[schemars(schema_with = "any_json_schema")]
    pub output: Option<serde_json::Value>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadMacroIndicatorParams {
    /// indicator の id。利用可能な id は search_refs で確認できる
    pub indicator_id: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: NaiveDate,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: NaiveDate,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct IndicatorObservationDto {
    pub date: NaiveDate,
    /// FRED から取得した原単位で返す
    pub value: f64,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadMacroIndicatorResult {
    pub indicator_id: String,
    /// 日付昇順。データが無ければ空配列
    pub observations: Vec<IndicatorObservationDto>,
}

mod short_selling;
mod valuation;
pub use short_selling::*;
pub use valuation::*;

#[cfg(test)]
mod tests {
    use super::WriteNoteParams;

    #[test]
    fn write_note_kind_deserialization_distinguishes_missing_null_and_value() {
        fn parse(json: &str) -> Option<Option<String>> {
            serde_json::from_str::<WriteNoteParams>(json)
                .expect("valid write_note parameters")
                .kind
        }

        assert_eq!(
            (
                parse("{}"),
                parse(r#"{"kind":null}"#),
                parse(r#"{"kind":"example-kind"}"#),
            ),
            (None, Some(None), Some(Some("example-kind".into()))),
        );
    }
}
