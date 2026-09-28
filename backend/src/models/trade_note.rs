use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use gateway_postgres::entities::trade_note;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = TradeNote)]
pub struct TradeNoteResponse {
    pub trade_id: Uuid,
    pub note_id: Uuid,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub note_version_id: Uuid,
}

impl From<trade_note::Model> for TradeNoteResponse {
    fn from(model: trade_note::Model) -> Self {
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
