use chrono::{DateTime, FixedOffset};
use core_application::custom_indicator_evaluation::CustomIndicatorEvaluation;
use serde::Serialize;
use serde_json::Value as Json;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct CustomIndicatorEvaluationResponse {
    pub id: Uuid,
    pub indicator_name: String,
    pub code: String,
    #[schema(value_type = serde_json::Value)]
    pub args: Json,
    #[schema(value_type = serde_json::Value)]
    pub output: Json,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub evaluated_at: DateTime<FixedOffset>,
}

impl From<CustomIndicatorEvaluation> for CustomIndicatorEvaluationResponse {
    fn from(evaluation: CustomIndicatorEvaluation) -> Self {
        Self {
            id: evaluation.id,
            indicator_name: evaluation.indicator_name,
            code: evaluation.code,
            args: evaluation.args,
            output: evaluation.output,
            evaluated_at: evaluation.evaluated_at,
        }
    }
}
