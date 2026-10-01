use chrono::{DateTime, FixedOffset};
use sea_orm::entity::prelude::Json;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use core_application::change_history::ChangeHistoryEntry;

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = ChangeHistory)]
pub struct ChangeHistoryResponse {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub actor_kind: String,
    pub actor_label: String,
    pub op: String,
    pub diff_json: Json,
    pub summary: Option<String>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
}

impl From<ChangeHistoryEntry> for ChangeHistoryResponse {
    fn from(model: ChangeHistoryEntry) -> Self {
        Self {
            id: model.id,
            target_kind: model.target_kind,
            target_id: model.target_id,
            actor_kind: model.actor_kind,
            actor_label: model.actor_label,
            op: model.op,
            diff_json: model.diff_json,
            summary: model.summary,
            created_at: model.created_at,
        }
    }
}
