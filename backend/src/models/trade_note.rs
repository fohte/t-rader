use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = TradeNote)]
pub struct TradeNoteResponse {
    pub trade_id: Uuid,
    pub note_id: Uuid,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub note_version_id: Uuid,
}

impl From<core_application::trade::TradeNoteLink> for TradeNoteResponse {
    fn from(model: core_application::trade::TradeNoteLink) -> Self {
        Self {
            trade_id: model.trade_id,
            note_id: model.note_id,
            created_at: model.created_at,
            note_version_id: model.note_version_id,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTradeNoteRequest {
    pub note_id: Uuid,
}
