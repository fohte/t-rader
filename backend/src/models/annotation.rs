use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAnnotationRequest {
    /// 任意。省略した場合、どの戦略にも属さないアノテーションになる (市況・セクター横断の分析など)
    pub strategy_id: Option<Uuid>,
    #[schema(min_length = 1)]
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<Decimal>,
    pub text: String,
    pub status: Option<String>,
    pub linked_note_id: Option<Uuid>,
    #[serde(default)]
    pub created_by_kind: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateAnnotationRequest {
    pub target_symbol: Option<String>,
    pub target_kind: Option<String>,
    pub timestamp: Option<DateTime<FixedOffset>>,
    pub price: Option<Decimal>,
    pub text: Option<String>,
    pub linked_note_id: Option<Uuid>,
}
