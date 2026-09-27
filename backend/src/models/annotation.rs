use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entities::annotation;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Annotation)]
pub struct AnnotationResponse {
    pub id: Uuid,
    pub strategy_id: Option<Uuid>,
    pub target_symbol: String,
    pub target_kind: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub timestamp: DateTime<FixedOffset>,
    pub price: Option<Decimal>,
    pub text: String,
    pub status: String,
    pub linked_note_id: Option<Uuid>,
    pub created_by_kind: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
    pub execution_step_id: Option<Uuid>,
    pub execution_task_id: Option<String>,
}

impl From<annotation::Model> for AnnotationResponse {
    fn from(model: annotation::Model) -> Self {
        Self {
            id: model.id,
            strategy_id: model.strategy_id,
            target_symbol: model.target_symbol,
            target_kind: model.target_kind,
            timestamp: model.timestamp,
            price: model.price,
            text: model.text,
            status: model.status,
            linked_note_id: model.linked_note_id,
            created_by_kind: model.created_by_kind,
            created_at: model.created_at,
            updated_at: model.updated_at,
            execution_step_id: model.execution_step_id,
            execution_task_id: model.execution_task_id,
        }
    }
}

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
