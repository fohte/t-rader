use serde::Serialize;
use utoipa::ToSchema;

use core_application::note_tag::NoteTag;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = NoteTag)]
pub struct NoteTagResponse {
    pub key: String,
    pub display_name: String,
    pub description: Option<String>,
}

impl From<NoteTag> for NoteTagResponse {
    fn from(tag: NoteTag) -> Self {
        Self {
            key: tag.key,
            display_name: tag.display_name,
            description: tag.description,
        }
    }
}
