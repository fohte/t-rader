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
    /// 対象銘柄コードの配列。1 回の呼び出しで複数銘柄をまとめて取得できる
    /// (最大 100 件、重複不可)
    pub instrument_ids: Vec<String>,
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

/// 1 銘柄分の日足バー。データが 1 件も無い銘柄は `bars: []` になる
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct InstrumentBarsDto {
    pub instrument_id: String,
    pub bars: Vec<BarDto>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct QueryDataResult {
    /// `instrument_ids` と同じ順序
    pub results: Vec<InstrumentBarsDto>,
}

/// 銘柄ごとの未決済ポジションと損益 (FIFO ベース)
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PortfolioPositionDto {
    pub symbol: String,
    /// 保有数量 (買い残 - 売り残)
    pub qty: f64,
    /// 平均取得単価 (FIFO ベース)
    pub avg_cost: f64,
    /// 取得簿価 (qty * avg_cost)
    pub cost_basis: f64,
    /// 実現損益累計
    pub realized_pnl: f64,
    /// 直近終値。取得できなかった場合は null
    pub current_price: Option<f64>,
    /// 保有時価 (qty * current_price)。current_price が null の場合は null
    pub market_value: Option<f64>,
    /// 含み損益 (market_value - cost_basis)。current_price が null の場合は null
    pub unrealized_pnl: Option<f64>,
}

/// 口座全体、または単一戦略のポジション集計
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PortfolioScopeDto {
    pub trade_count: i64,
    /// 全銘柄合計の実現損益
    pub realized_pnl: f64,
    /// 保有時価合計 (current_price が取れたポジションのみの合計)
    pub market_value: f64,
    pub positions: Vec<PortfolioPositionDto>,
}

/// 戦略単位のポジション集計 + 投資可能額
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct StrategyPortfolioScopeDto {
    pub trade_count: i64,
    pub realized_pnl: f64,
    pub market_value: f64,
    pub positions: Vec<PortfolioPositionDto>,
    /// 戦略に割り当てられた投資可能額の現在値。記録が無ければ null
    pub investable_amount: Option<f64>,
    /// 投資可能額 + 実現損益 - 取得原価。investable_amount が null の場合は null
    pub unused_investable_amount: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadPortfolioResult {
    /// 保有時価の評価に用いた対象営業日。current_price を持つ全ポジションに共通する日付
    /// (日付が食い違う銘柄は current_price が null になる)。1 銘柄も評価できなければ null
    pub priced_at: Option<NaiveDate>,
    /// 口座全体 (全戦略横断) の集計
    pub account: PortfolioScopeDto,
    /// 接続元戦略の集計
    pub strategy: StrategyPortfolioScopeDto,
}

/// 個々の約定 (account-wide、全戦略横断)
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct TradeDto {
    pub trade_id: Uuid,
    pub strategy_id: Uuid,
    /// 約定日
    pub date: NaiveDate,
    pub symbol: String,
    /// "buy" | "sell"
    pub side: String,
    pub qty: f64,
    pub price: f64,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ReadTradesParams {
    /// この銘柄コードに一致する取引のみ返す。省略時は全銘柄
    pub symbol: Option<String>,
    /// この約定日以降 (inclusive) の取引のみ返す。省略時は下限なし
    pub date_from: Option<NaiveDate>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadTradesResult {
    pub trades: Vec<TradeDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckBuyableQtyParams {
    /// 対象銘柄コード (例: "7203")
    pub symbol: String,
}

/// 制約単位の追加購入可能株数。
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConstraintResult {
    /// 上限が効いている。`max_additional_qty` は `lot_size` の倍数に切り捨て済み
    Limited { max_additional_qty: i64 },
    /// risk_policy でこの制約自体が未設定 (上限なし)
    Unlimited,
    /// 計算に必要な値が欠けており判定できない
    Unavailable { reason: String },
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CheckBuyableQtyResult {
    pub symbol: String,
    /// 単元株数。各上限株数はこの倍数に切り捨てて返す
    pub lot_size: i64,
    /// 接続元戦略が現在保有する株数。保有していなければ 0
    pub current_qty: f64,
    /// 直近終値。取得できなかった場合は null (この場合、価格に依存する制約はすべて unavailable になる)
    pub current_price: Option<f64>,
    /// 価格取得を試みた銘柄 (口座全体の保有銘柄 + 対象銘柄) のうち、取得できたもので
    /// 最も新しい観測日。1 銘柄も取得できなければ null。`current_price` 自体の観測日とは
    /// 限らない
    pub priced_at: Option<NaiveDate>,
    /// `account_risk_policy.max_sector_ratio` による制約
    pub max_qty_by_sector_ratio: ConstraintResult,
    /// 戦略の未使用投資可能額による制約
    pub max_qty_by_cash: ConstraintResult,
    /// 上記制約のうち最も厳しいもの。いずれかが unavailable なら unavailable、
    /// 全て unlimited なら unlimited
    pub max_qty: ConstraintResult,
    /// `max_qty` が `Limited` のとき、根拠になった制約名
    /// (`sector_ratio` / `cash`)。それ以外は null
    pub binding_constraint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
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
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
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

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct WriteNoteResult {
    pub note_id: Uuid,
    pub created: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadNoteParams {
    pub note_id: Uuid,
    /// 省略時は現行バージョンを読む。
    pub version_id: Option<Uuid>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct NoteLinkDto {
    /// 参照先ノート ID。
    pub to_note_id: Uuid,
    /// 固定したバージョン ID。null の場合は参照先ノートの現行バージョンに追従する。
    pub to_version_id: Option<Uuid>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct NoteDto {
    pub note_id: Uuid,
    pub strategy_id: Uuid,
    /// 本文が属するバージョン ID。`read_comments` の `target_id` に使う。
    pub version_id: Uuid,
    /// ノート内のバージョン番号。
    pub version_no: i32,
    pub title: String,
    /// `list_notes` で `include_body: false` を指定したときのみ省略される (null)。
    /// `read_note` の結果では常に値を含む
    pub body_md: Option<String>,
    pub frontmatter_json: serde_json::Map<String, serde_json::Value>,
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

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct NoteKindDto {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ListNoteKindsResult {
    pub note_kinds: Vec<NoteKindDto>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListNotesParams {
    pub limit: Option<u32>,
    /// "approved" / "unread" / "rejected" のいずれかで絞り込む。省略時は全 status
    pub status: Option<String>,
    /// この時刻以降 (inclusive) に更新されたノートのみ返す。省略時は下限なし
    pub updated_after: Option<DateTime<FixedOffset>>,
    /// false を指定すると body_md を省略し、レスポンスサイズを抑える。省略時は true (本文を含む)
    pub include_body: Option<bool>,
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
pub struct ReadCommentsParams {
    /// "note_version" | "annotation"
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
    /// 行コメントが対応する本文側。`note_version` の場合のみ設定される。
    pub anchor_side: Option<String>,
    /// 対応する本文中の行位置 (1-indexed)。
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchWebParams {
    /// 検索したい内容を表す自然文の問い合わせ
    pub query: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SearchWebResult {
    pub text: String,
    /// 出典 URL。重複除去済み
    pub citations: Vec<String>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchNewsParams {
    /// title / body_snippet の部分一致 (大文字小文字を区別しない)。省略時はキーワード条件なし
    pub keyword: Option<String>,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

/// `search_news` で返す記事 1 件
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct NewsItemDto {
    pub id: Uuid,
    pub source: String,
    pub url: String,
    pub title: String,
    pub body_snippet: Option<String>,
    pub published_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SearchNewsResult {
    pub items: Vec<NewsItemDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFinSummaryParams {
    /// 対象銘柄コード (4桁、例: "7203")
    pub symbol: String,
    pub limit: Option<u32>,
}

/// 財務情報テーブルの 1 開示分。記載の無い項目は null。
/// IFRS/米国基準では ordinary_profit (経常利益) が概念自体存在せず null になる。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct FinSummaryDto {
    /// 開示日
    pub disc_date: NaiveDate,
    /// 開示書類種別 (例: "FYFinancialStatements_Consolidated_JP", "EarnForecastRevision")
    pub doc_type: Option<String>,
    /// 当会計期間の種類 (1Q/2Q/3Q/4Q/5Q/FY)
    pub current_period_type: Option<String>,
    pub current_period_start: Option<NaiveDate>,
    pub current_period_end: Option<NaiveDate>,
    pub current_fiscal_year_start: Option<NaiveDate>,
    pub current_fiscal_year_end: Option<NaiveDate>,
    /// 売上高 (実績)
    pub sales: Option<f64>,
    /// 営業利益 (実績)
    pub operating_profit: Option<f64>,
    /// 経常利益 (実績)。IFRS/米国基準では null
    pub ordinary_profit: Option<f64>,
    /// 当期純利益 (実績)
    pub net_profit: Option<f64>,
    /// 売上高の進捗率 (四半期累計実績 ÷ 同じ開示行の当期通期会社予想。0.5 は 50%)
    pub sales_progress_rate: Option<f64>,
    /// 営業利益の進捗率 (四半期累計実績 ÷ 同じ開示行の当期通期会社予想。0.5 は 50%)
    pub operating_profit_progress_rate: Option<f64>,
    /// 経常利益の進捗率 (四半期累計実績 ÷ 同じ開示行の当期通期会社予想。0.5 は 50%)
    pub ordinary_profit_progress_rate: Option<f64>,
    /// 当期純利益の進捗率 (四半期累計実績 ÷ 同じ開示行の当期通期会社予想。0.5 は 50%)
    pub net_profit_progress_rate: Option<f64>,
    /// 1 株当たり当期純利益 (実績)
    pub eps: Option<f64>,
    /// 1 株当たり純資産 (実績)
    pub bps: Option<f64>,
    /// 総資産 (実績)
    pub total_assets: Option<f64>,
    /// 純資産 (実績)
    pub equity: Option<f64>,
    /// 自己資本比率 (実績)
    pub equity_to_asset_ratio: Option<f64>,
    /// 自己資本利益率 (実績)
    pub roe: Option<f64>,
    pub cf_operating: Option<f64>,
    pub cf_investing: Option<f64>,
    pub cf_financing: Option<f64>,
    pub cash_and_equivalents: Option<f64>,
    /// 年間配当実績 (1 株当たり合計)
    pub dividend_annual: Option<f64>,
    /// 年間配当予想 (当事業年度、1 株当たり合計)
    pub dividend_annual_forecast: Option<f64>,
    /// 年間配当予想 (翌事業年度、1 株当たり合計)
    pub dividend_annual_forecast_next: Option<f64>,
    /// 会社予想 売上高 (当期通期)
    pub forecast_sales: Option<f64>,
    /// 会社予想 営業利益 (当期通期)
    pub forecast_operating_profit: Option<f64>,
    /// 会社予想 経常利益 (当期通期)
    pub forecast_ordinary_profit: Option<f64>,
    /// 会社予想 当期純利益 (当期通期)
    pub forecast_net_profit: Option<f64>,
    /// 会社予想 1 株当たり当期純利益 (当期通期)
    pub forecast_eps: Option<f64>,
    /// 会社予想 売上高 (翌期通期)
    pub next_forecast_sales: Option<f64>,
    /// 会社予想 営業利益 (翌期通期)
    pub next_forecast_operating_profit: Option<f64>,
    /// 会社予想 経常利益 (翌期通期)
    pub next_forecast_ordinary_profit: Option<f64>,
    /// 会社予想 当期純利益 (翌期通期)
    pub next_forecast_net_profit: Option<f64>,
    /// 会社予想 1 株当たり当期純利益 (翌期通期)
    pub next_forecast_eps: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadFinSummaryResult {
    pub items: Vec<FinSummaryDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListHypothesesParams {
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct HypothesisDto {
    pub hypothesis_id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub title: String,
    pub body: String,
    pub status: String,
    pub related_note_ids: Vec<Uuid>,
    pub related_interest_ids: Vec<Uuid>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListHypothesesResult {
    pub hypotheses: Vec<HypothesisDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadHypothesisParams {
    pub hypothesis_id: Uuid,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProposeHypothesisChangeParams {
    pub hypothesis_id: Uuid,
    pub proposed_title: Option<String>,
    pub proposed_body: Option<String>,
    pub proposed_status: Option<String>,
    /// なぜこの変更を提案するかの根拠。人間のレビュー時に必須で参照される
    pub rationale: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ProposeHypothesisChangeResult {
    pub proposal_id: Uuid,
    pub status: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordPredictionParams {
    /// 根拠となるノート (自戦略所有のもの)。省略可
    pub note_id: Option<Uuid>,
    /// 対象銘柄コード
    pub target_stock_id: String,
    /// 比較対象の銘柄コード (例: TOPIX 連動 ETF)
    pub benchmark_stock_id: String,
    /// 対象が比較対象を上回るか下回るか (`outperform` / `underperform`)
    pub direction: String,
    /// 固定刻み (0.55/0.6/0.65/0.7/0.75/0.8/0.85/0.9) のいずれかのみ受け付ける
    pub probability: f64,
    /// この日の終値を基準とする (YYYY-MM-DD)
    pub base_date: NaiveDate,
    /// 期限日 (YYYY-MM-DD)。base_date より後である必要がある
    pub due_date: NaiveDate,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
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

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct RecordPredictionResult {
    pub prediction: PredictionDto,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListPredictionsParams {
    pub limit: Option<u32>,
    /// 期限がこの日付以降 (inclusive) の予測のみ返す。「まだ結果が出ていない予測」を絞るときに使う
    pub due_after: Option<NaiveDate>,
    /// 期限がこの日付以前 (inclusive) の予測のみ返す
    pub due_before: Option<NaiveDate>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListPredictionsResult {
    pub predictions: Vec<PredictionDto>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PredictionProbabilityBucketDto {
    /// 記録時の確率刻み (0.55〜0.90)
    pub probability: f64,
    /// この確率刻みで採点済みの予測件数
    pub count: u32,
    /// この確率刻みでの的中率 (的中件数 / count)。count が 0 の場合は null
    pub hit_rate: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadPredictionStatsResult {
    /// 採点済み予測の件数
    pub graded_count: u32,
    /// Brier score: 確率 p と的中 (1) / 非的中 (0) の二乗誤差の平均。低いほど較正が良い。
    /// 採点済み予測が 1 件も無ければ null
    pub brier_score: Option<f64>,
    /// 確率刻みごとの集計。刻み昇順 (0.55 → 0.90)。件数 0 の刻みも含む
    pub buckets: Vec<PredictionProbabilityBucketDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadShareholdingStructureParams {
    /// 4桁の銘柄コード (例: "7203")
    pub symbol: String,
    /// large_volume_reports の返却件数上限
    pub limit: Option<u32>,
}

/// 大量保有報告書 / 変更報告書の書類種別 (EDINET `LargeHldgTypeCode`)
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LargeVolumeDocumentType {
    LargeVolumeReport,
    Amendment,
    AmendmentRapidTransfer,
    LargeVolumeReportSpecial,
    AmendmentSpecial,
    Unknown,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct LargeVolumeHolderDto {
    pub holder_name: String,
    pub holding_purpose: Option<String>,
    pub shares_held: Option<i64>,
    /// 保有割合 (小数。0.1 = 10%)
    pub shares_ratio: Option<f64>,
    /// 直前の報告における保有割合 (変更報告書のみ)
    pub shares_ratio_last: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct LargeVolumeReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    pub document_type: LargeVolumeDocumentType,
    /// 変更事由 (変更報告書のみ)
    pub change_reason: Option<String>,
    /// 保有割合合計 (小数。0.1 = 10%)
    pub total_shares_ratio: Option<f64>,
    /// 直前の報告における保有割合合計 (変更報告書のみ)
    pub total_shares_ratio_last: Option<f64>,
    pub holders: Vec<LargeVolumeHolderDto>,
}

/// 大株主状況の書類種別 (EDINET `DocTypeCode`)
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MajorShareholdersDocumentType {
    AnnualReport,
    QuarterlyReport,
    SemiAnnualReport,
    Unknown,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MajorShareholderDto {
    /// 順位。通常 1-10 位だが件数は書類ごとに異なる
    pub rank: Option<i32>,
    pub holder_name: String,
    pub shares_held: Option<i64>,
    /// 発行済株式に対する所有割合 (小数。0.1 = 10%)
    pub shares_ratio: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MajorShareholdersReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    /// 事業年度末
    pub period_end: Option<NaiveDate>,
    pub document_type: MajorShareholdersDocumentType,
    /// 順位昇順
    pub holders: Vec<MajorShareholderDto>,
}

/// 政策保有株式の種別 (EDINET の `Spec`/`Deem` 配列の別)
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CrossShareholdingCategory {
    /// 特定投資株式
    Specified,
    /// みなし保有株式
    Deemed,
}

/// 保有先が当社株式を保有しているか (EDINET `IsrHoldsCode`)
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutualHolding {
    Held,
    NotHeld,
    Unknown,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CrossShareholdingDto {
    /// 保有先の会社名
    pub issuer_name: String,
    /// 保有先の銘柄コード (5桁、J-Quants による名寄せ済み)。判別できない場合は null
    pub issuer_code: Option<String>,
    pub category: CrossShareholdingCategory,
    /// 当事業年度末の保有株式数
    pub current_shares: Option<i64>,
    /// 前事業年度末の保有株式数
    pub previous_shares: Option<i64>,
    /// 当事業年度末の貸借対照表計上額 (円)
    pub current_book_value: Option<i64>,
    /// 前事業年度末の貸借対照表計上額 (円)
    pub previous_book_value: Option<i64>,
    pub mutual_holding: MutualHolding,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CrossShareholdingsReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    /// 事業年度末
    pub period_end: Option<NaiveDate>,
    pub holdings: Vec<CrossShareholdingDto>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadShareholdingStructureResult {
    pub symbol: String,
    /// 大量保有報告書 + 変更報告書。新しい順。取り込み済みデータが無ければ空配列
    pub large_volume_reports: Vec<LargeVolumeReportDto>,
    /// 直近の大株主状況。取り込み済みデータが無ければ null
    pub major_shareholders: Option<MajorShareholdersReportDto>,
    /// 直近の政策保有株式 (自社が保有する側、保有先ごと)。取り込み済みデータが無ければ null
    pub cross_shareholdings: Option<CrossShareholdingsReportDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMacroIndicatorParams {
    /// indicator の id (例: "USDJPY", "VIX", "US10Y", "NIKKEI225")。search_refs で発見できる
    pub indicator_id: String,
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: NaiveDate,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: NaiveDate,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct IndicatorObservationDto {
    pub date: NaiveDate,
    /// FRED 由来の単位そのまま (例: USDJPY は 1 ドルあたりの円、US10Y は %)
    pub value: f64,
}

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
