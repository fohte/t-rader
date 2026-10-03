use serde::Serialize;
use utoipa::ToSchema;

use core_application::note_kind::NoteKind;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = NoteKind)]
pub struct NoteKindResponse {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

impl From<NoteKind> for NoteKindResponse {
    fn from(kind: NoteKind) -> Self {
        Self {
            key: kind.key,
            display_name: kind.display_name,
            requires_approval: kind.requires_approval,
            description: kind.description,
            sort_order: kind.sort_order,
        }
    }
}
