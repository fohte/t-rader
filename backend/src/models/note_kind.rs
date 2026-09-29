use serde::Serialize;
use utoipa::ToSchema;

use gateway_postgres::entities::note_kind;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = NoteKind)]
pub struct NoteKindResponse {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

impl From<note_kind::Model> for NoteKindResponse {
    fn from(kind: note_kind::Model) -> Self {
        Self {
            key: kind.key,
            display_name: kind.display_name,
            requires_approval: kind.requires_approval,
            description: kind.description,
            sort_order: kind.sort_order,
        }
    }
}
