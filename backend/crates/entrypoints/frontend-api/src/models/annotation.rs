use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Annotation)]
pub struct AnnotationResponse {
    pub id: Uuid,
    pub target_symbol: String,
    pub target_kind: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub timestamp: DateTime<FixedOffset>,
    #[schema(value_type = Option<chrono::DateTime<chrono::Utc>>)]
    pub timestamp_start: Option<DateTime<FixedOffset>>,
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

impl From<core_application::annotation::Annotation> for AnnotationResponse {
    fn from(model: core_application::annotation::Annotation) -> Self {
        Self {
            id: model.id,
            target_symbol: model.target_symbol,
            target_kind: model.target_kind,
            timestamp: model.timestamp,
            timestamp_start: model.timestamp_start,
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

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use serde_json::json;
    use uuid::Uuid;

    use super::AnnotationResponse;

    #[test]
    fn annotation_response_serializes_timestamp_start() {
        let timestamp =
            DateTime::parse_from_rfc3339("2030-01-02T00:00:00Z").expect("valid timestamp");
        let timestamp_start =
            DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z").expect("valid timestamp");
        let response = AnnotationResponse::from(core_application::annotation::Annotation {
            id: Uuid::nil(),
            target_symbol: "FICTIONAL-ASSET".into(),
            target_kind: "test-kind".into(),
            timestamp,
            timestamp_start: Some(timestamp_start),
            price: None,
            text: "sample annotation".into(),
            status: "unread".into(),
            linked_note_id: None,
            created_by_kind: "llm".into(),
            created_at: timestamp,
            updated_at: timestamp,
            execution_step_id: None,
            execution_task_id: None,
        });

        assert_eq!(
            serde_json::to_value(response).expect("response serializes"),
            json!({
                "id": Uuid::nil(),
                "target_symbol": "FICTIONAL-ASSET",
                "target_kind": "test-kind",
                "timestamp": timestamp,
                "timestamp_start": timestamp_start,
                "price": null,
                "text": "sample annotation",
                "status": "unread",
                "linked_note_id": null,
                "created_by_kind": "llm",
                "created_at": timestamp,
                "updated_at": timestamp,
                "execution_step_id": null,
                "execution_task_id": null,
            }),
        );
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateAnnotationRequest {
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
