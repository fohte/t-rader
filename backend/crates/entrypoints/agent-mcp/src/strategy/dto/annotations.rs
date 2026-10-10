use chrono::{DateTime, FixedOffset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAnnotationParams {
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    /// アノテーション自身が語る期間の開始時刻
    pub timestamp_start: Option<DateTime<FixedOffset>>,
    /// この実行で取得した日足 (1d) の query_data から解決する価格項目
    pub price_field: Option<AnnotationPriceField>,
    pub text: String,
    pub linked_note_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationPriceField {
    Open,
    High,
    Low,
    Close,
}

#[cfg_attr(test, derive(Clone, serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct AnnotationDto {
    pub annotation_id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    pub timestamp: DateTime<FixedOffset>,
    pub timestamp_start: Option<DateTime<FixedOffset>>,
    pub price: Option<f64>,
    pub text: String,
    pub status: String,
    pub linked_note_id: Option<Uuid>,
    pub created_by_kind: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct CreateAnnotationResult {
    pub annotation: AnnotationDto,
    /// 作成は成功したが、入力の確認が必要な場合の警告。
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadAnnotationsParams {
    pub target_symbol: Option<String>,
    pub limit: Option<u32>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadAnnotationsResult {
    pub annotations: Vec<AnnotationDto>,
}
