use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use gateway_postgres::entities::prediction;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Prediction)]
pub struct PredictionResponse {
    pub prediction_id: Uuid,
    pub strategy_id: Uuid,
    pub note_id: Option<Uuid>,
    pub target_stock_id: String,
    pub benchmark_stock_id: String,
    pub direction: String,
    pub probability: Decimal,
    pub base_date: NaiveDate,
    pub due_date: NaiveDate,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
}

impl From<prediction::Model> for PredictionResponse {
    fn from(prediction: prediction::Model) -> Self {
        Self {
            prediction_id: prediction.prediction_id,
            strategy_id: prediction.strategy_id,
            note_id: prediction.note_id,
            target_stock_id: prediction.target_stock_id,
            benchmark_stock_id: prediction.benchmark_stock_id,
            direction: prediction.direction,
            probability: prediction.probability,
            base_date: prediction.base_date,
            due_date: prediction.due_date,
            created_at: prediction.created_at,
        }
    }
}
