use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::graph_dto::GraphDef;

#[derive(Debug, Deserialize, Serialize)]
pub struct QueryDataParams {
    pub instrument_ids: Vec<String>,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct BarDto {
    pub timestamp: DateTime<FixedOffset>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct InstrumentBarsDto {
    pub instrument_id: String,
    pub bars: Vec<BarDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct QueryDataResult {
    pub results: Vec<InstrumentBarsDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct PortfolioPositionDto {
    pub symbol: String,
    pub qty: f64,
    pub avg_cost: f64,
    pub cost_basis: f64,
    pub realized_pnl: f64,
    pub current_price: Option<f64>,
    pub market_value: Option<f64>,
    pub unrealized_pnl: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct PortfolioScopeDto {
    pub trade_count: i64,
    pub realized_pnl: f64,
    pub market_value: f64,
    pub positions: Vec<PortfolioPositionDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct StrategyPortfolioScopeDto {
    pub trade_count: i64,
    pub realized_pnl: f64,
    pub market_value: f64,
    pub positions: Vec<PortfolioPositionDto>,
    pub investable_amount: Option<f64>,
    pub unused_investable_amount: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ReadPortfolioResult {
    pub priced_at: Option<NaiveDate>,
    pub account: PortfolioScopeDto,
    pub strategy: StrategyPortfolioScopeDto,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct TradeDto {
    pub trade_id: Uuid,
    pub strategy_id: Uuid,
    pub date: NaiveDate,
    pub symbol: String,
    pub side: String,
    pub qty: f64,
    pub price: f64,
    pub notes: Vec<TradeNoteReferenceDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TradeNoteReferenceDto {
    pub note_id: Uuid,
    pub note_version_id: Uuid,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ReadTradesParams {
    pub symbol: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ReadTradesResult {
    pub trades: Vec<TradeDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CheckBuyableQtyParams {
    pub symbol: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConstraintResult {
    Limited { max_additional_qty: i64 },
    Unlimited,
    Unavailable { reason: String },
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CheckBuyableQtyResult {
    pub symbol: String,
    pub lot_size: i64,
    pub current_qty: f64,
    pub current_price: Option<f64>,
    pub priced_at: Option<NaiveDate>,
    pub max_qty_by_group_ratios: ConstraintResult,
    pub max_qty_by_cash: ConstraintResult,
    pub max_qty: ConstraintResult,
    pub binding_constraint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WriteNoteParams {
    pub note_id: Option<Uuid>,
    pub title: Option<String>,
    pub body_md: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Option<String>>,
    pub change_reason: Option<String>,
    pub frontmatter_json: Option<serde_json::Map<String, serde_json::Value>>,
    pub graphs: Option<Vec<GraphDef>>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WriteNoteResult {
    pub note_id: Uuid,
    pub created: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadNoteParams {
    pub note_id: Uuid,
    pub version_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct NoteLinkDto {
    pub to_note_id: Uuid,
    pub to_version_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct NoteDto {
    pub note_id: Uuid,
    pub version_id: Uuid,
    pub version_no: i32,
    pub title: String,
    pub body_md: Option<String>,
    pub frontmatter_json: serde_json::Map<String, serde_json::Value>,
    pub tags: Vec<String>,
    pub kind: Option<String>,
    pub status: String,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub graphs: Vec<GraphDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<NoteLinkDto>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ListNotesParams {
    pub limit: Option<u32>,
    pub kind: Option<String>,
    #[serde(rename = "ref")]
    pub r#ref: Option<String>,
    pub status: Option<String>,
    pub updated_after: Option<DateTime<FixedOffset>>,
    pub include_body: Option<bool>,
    pub include_pending: Option<bool>,
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ListNotesResult {
    pub notes: Vec<NoteDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateAnnotationParams {
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<f64>,
    pub text: String,
    pub linked_note_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AnnotationDto {
    pub annotation_id: Uuid,
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

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CreateAnnotationResult {
    pub annotation: AnnotationDto,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadAnnotationsParams {
    pub target_symbol: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ReadAnnotationsResult {
    pub annotations: Vec<AnnotationDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadCommentsParams {
    pub target_kind: String,
    pub target_id: Uuid,
    pub resolved: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CommentDto {
    pub comment_id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub author_kind: String,
    pub author_label: String,
    pub resolved: bool,
    pub created_at: DateTime<FixedOffset>,
    pub anchor_text: Option<String>,
    pub anchor_side: Option<String>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReadCommentsResult {
    pub comments: Vec<CommentDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResolveCommentParams {
    pub comment_id: Uuid,
    pub resolved: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ResolveCommentResult {
    pub comment: CommentDto,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReplyCommentParams {
    pub parent_id: Uuid,
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReplyCommentResult {
    pub comment: CommentDto,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EvalPythonParams {
    pub code: String,
    pub stdin: Option<String>,
    pub timeout_secs: Option<u32>,
    pub max_output_bytes: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct EvalPythonResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EvalIndicatorParams {
    pub name: String,
    pub args: serde_json::Value,
    pub timeout_secs: Option<u32>,
    pub max_output_bytes: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueryYoutubeParams {
    pub youtube_url: String,
    pub questions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct QueryYoutubeResult {
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchWebParams {
    pub query: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchWebResult {
    pub text: String,
    pub citations: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct EvalIndicatorResult {
    pub indicator_id: Uuid,
    pub scope: String,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchNewsParams {
    pub keyword: Option<String>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct NewsItemDto {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub body_snippet: Option<String>,
    pub content_status: Option<String>,
    pub published_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SearchNewsResult {
    pub items: Vec<NewsItemDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetNewsContentParams {
    pub id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct GetNewsContentResult {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub published_at: DateTime<FixedOffset>,
    pub content_status: Option<String>,
    pub content: Option<String>,
    pub content_error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RecordPredictionParams {
    pub note_id: Option<Uuid>,
    pub target_stock_id: String,
    pub benchmark_stock_id: String,
    pub direction: String,
    pub probability: f64,
    pub base_date: NaiveDate,
    pub due_date: NaiveDate,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct PredictionDto {
    pub prediction_id: Uuid,
    pub strategy_id: Uuid,
    pub note_id: Option<Uuid>,
    pub target_stock_id: String,
    pub benchmark_stock_id: String,
    pub direction: String,
    pub probability: f64,
    pub base_date: NaiveDate,
    pub due_date: NaiveDate,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct RecordPredictionResult {
    pub prediction: PredictionDto,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListPredictionsParams {
    pub limit: Option<u32>,
    pub due_after: Option<NaiveDate>,
    pub due_before: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ListPredictionsResult {
    pub predictions: Vec<PredictionDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct PredictionProbabilityBucketDto {
    pub probability: f64,
    pub count: u32,
    pub hit_rate: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ReadPredictionStatsResult {
    pub graded_count: u32,
    pub brier_score: Option<f64>,
    pub buckets: Vec<PredictionProbabilityBucketDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadShareholdingStructureParams {
    pub symbol: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LargeVolumeDocumentType {
    LargeVolumeReport,
    Amendment,
    AmendmentRapidTransfer,
    LargeVolumeReportSpecial,
    AmendmentSpecial,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct LargeVolumeHolderDto {
    pub holder_name: String,
    pub holding_purpose: Option<String>,
    pub shares_held: Option<i64>,
    pub shares_ratio: Option<f64>,
    pub shares_ratio_last: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct LargeVolumeReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    pub document_type: LargeVolumeDocumentType,
    pub change_reason: Option<String>,
    pub total_shares_ratio: Option<f64>,
    pub total_shares_ratio_last: Option<f64>,
    pub holders: Vec<LargeVolumeHolderDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MajorShareholdersDocumentType {
    AnnualReport,
    QuarterlyReport,
    SemiAnnualReport,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct MajorShareholderDto {
    pub rank: Option<i32>,
    pub holder_name: String,
    pub shares_held: Option<i64>,
    pub shares_ratio: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct MajorShareholdersReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    pub period_end: Option<NaiveDate>,
    pub document_type: MajorShareholdersDocumentType,
    pub holders: Vec<MajorShareholderDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CrossShareholdingCategory {
    Specified,
    Deemed,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutualHolding {
    Held,
    NotHeld,
    Unknown,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CrossShareholdingDto {
    pub issuer_name: String,
    pub issuer_code: Option<String>,
    pub category: CrossShareholdingCategory,
    pub current_shares: Option<i64>,
    pub previous_shares: Option<i64>,
    pub current_book_value: Option<i64>,
    pub previous_book_value: Option<i64>,
    pub mutual_holding: MutualHolding,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct CrossShareholdingsReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    pub period_end: Option<NaiveDate>,
    pub holdings: Vec<CrossShareholdingDto>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ReadShareholdingStructureResult {
    pub symbol: String,
    pub large_volume_reports: Vec<LargeVolumeReportDto>,
    pub major_shareholders: Option<MajorShareholdersReportDto>,
    pub cross_shareholdings: Option<CrossShareholdingsReportDto>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadMacroIndicatorParams {
    pub indicator_id: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct IndicatorObservationDto {
    pub date: NaiveDate,
    pub value: f64,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ReadMacroIndicatorResult {
    pub indicator_id: String,
    pub observations: Vec<IndicatorObservationDto>,
}

mod short_selling;
pub use short_selling::*;
