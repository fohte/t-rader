use chrono::{DateTime, FixedOffset, NaiveDate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadFinSummaryParams {
    /// 対象銘柄コード (4桁、例: "7203")
    pub symbol: String,
    pub limit: Option<u32>,
}

/// 財務情報テーブルの 1 開示分。記載の無い項目は null。
/// IFRS/米国基準では ordinary_profit (経常利益) が概念自体存在せず null になる。
#[cfg_attr(test, derive(serde::Deserialize))]
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

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadFinSummaryResult {
    pub items: Vec<FinSummaryDto>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RecordPredictionParams {
    /// 根拠となるノート (どの戦略のノートでも指定可)。省略可
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

#[cfg_attr(test, derive(serde::Deserialize))]
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

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct RecordPredictionResult {
    pub prediction: PredictionDto,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ListPredictionsParams {
    pub limit: Option<u32>,
    /// 期限がこの日付以降 (inclusive) の予測のみ返す。「まだ結果が出ていない予測」を絞るときに使う
    pub due_after: Option<NaiveDate>,
    /// 期限がこの日付以前 (inclusive) の予測のみ返す
    pub due_before: Option<NaiveDate>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ListPredictionsResult {
    pub predictions: Vec<PredictionDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct PredictionProbabilityBucketDto {
    /// 記録時の確率刻み (0.55〜0.90)
    pub probability: f64,
    /// この確率刻みで採点済みの予測件数
    pub count: u32,
    /// この確率刻みでの的中率 (的中件数 / count)。count が 0 の場合は null
    pub hit_rate: Option<f64>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadShareholdingStructureParams {
    /// 4桁の銘柄コード (例: "7203")
    pub symbol: String,
    /// large_volume_reports の返却件数上限
    pub limit: Option<u32>,
}

/// 大量保有報告書 / 変更報告書の書類種別 (EDINET `LargeHldgTypeCode`)
#[cfg_attr(test, derive(serde::Deserialize))]
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

#[cfg_attr(test, derive(serde::Deserialize))]
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

#[cfg_attr(test, derive(serde::Deserialize))]
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
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MajorShareholdersDocumentType {
    AnnualReport,
    QuarterlyReport,
    SemiAnnualReport,
    Unknown,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct MajorShareholderDto {
    /// 順位。通常 1-10 位だが件数は書類ごとに異なる
    pub rank: Option<i32>,
    pub holder_name: String,
    pub shares_held: Option<i64>,
    /// 発行済株式に対する所有割合 (小数。0.1 = 10%)
    pub shares_ratio: Option<f64>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
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
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CrossShareholdingCategory {
    /// 特定投資株式
    Specified,
    /// みなし保有株式
    Deemed,
}

/// 保有先が当社株式を保有しているか (EDINET `IsrHoldsCode`)
#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MutualHolding {
    Held,
    NotHeld,
    Unknown,
}

#[cfg_attr(test, derive(serde::Deserialize))]
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

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CrossShareholdingsReportDto {
    pub doc_id: String,
    pub submitted_on: NaiveDate,
    /// 事業年度末
    pub period_end: Option<NaiveDate>,
    pub holdings: Vec<CrossShareholdingDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
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
