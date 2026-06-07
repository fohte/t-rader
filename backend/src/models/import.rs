use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// SBI CSV preview の 1 行。
#[derive(Debug, Serialize, ToSchema)]
pub struct SbiPreviewRow {
    /// 元 CSV 上の行番号 (0-based)
    pub row_index: usize,
    pub date: NaiveDate,
    pub symbol: String,
    pub stock_name: String,
    /// "buy" | "sell"
    pub side: String,
    #[schema(value_type = f64)]
    pub qty: Decimal,
    #[schema(value_type = f64)]
    pub price: Decimal,
    #[schema(value_type = f64)]
    pub fee: Decimal,
    /// 同日・同銘柄・同売買・同数量・同単価で既存取引が見つかったか
    pub is_duplicate: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SbiPreviewIssue {
    pub row_index: usize,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SbiPreviewResponse {
    pub rows: Vec<SbiPreviewRow>,
    pub issues: Vec<SbiPreviewIssue>,
}

/// SBI commit リクエストの 1 行。preview を確認後、行ごとに戦略 ID を割り当てる。
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SbiCommitRow {
    pub strategy_id: Uuid,
    pub date: NaiveDate,
    pub symbol: String,
    #[serde(default)]
    pub stock_name: Option<String>,
    pub side: String,
    #[schema(value_type = f64)]
    pub qty: Decimal,
    #[schema(value_type = f64)]
    pub price: Decimal,
    #[serde(default)]
    #[schema(value_type = Option<f64>)]
    pub fee: Option<Decimal>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SbiCommitRequest {
    pub rows: Vec<SbiCommitRow>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SbiCommitResponse {
    pub imported_count: usize,
    /// 重複検知でスキップした件数
    pub skipped_count: usize,
}
