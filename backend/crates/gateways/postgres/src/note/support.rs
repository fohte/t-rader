use core_application::note::{Note, NoteRepositoryError, NoteVersion};
use sea_orm::ActiveValue::{NotSet, Set};

use crate::entities::{note, note_version};
use crate::persistence::persistence_error;

#[derive(Clone, Copy, Default)]
pub struct PostgresNoteRepository;

impl PostgresNoteRepository {
    pub fn new() -> Self {
        Self
    }
}

pub(super) fn active_value(value: Option<Option<String>>) -> sea_orm::ActiveValue<Option<String>> {
    value.map_or(NotSet, Set)
}

pub(super) fn repository_error(error: sea_orm::DbErr) -> NoteRepositoryError {
    NoteRepositoryError::Database(persistence_error(error))
}

pub(super) fn to_note(model: note::Model) -> Note {
    Note {
        id: model.id,
        kind: model.kind,
        trigger: model.trigger,
        trigger_label: model.trigger_label,
        created_at: model.created_at,
        updated_at: model.updated_at,
        execution_id: model.execution_id,
    }
}

pub(super) fn to_version(model: note_version::Model) -> NoteVersion {
    NoteVersion {
        id: model.id,
        note_id: model.note_id,
        version_no: model.version_no,
        title: model.title,
        body_md: model.body_md,
        frontmatter_json: model.frontmatter_json,
        graphs_json: model.graphs_json,
        resolved_price_references_json: model.resolved_price_references_json,
        status: model.status,
        is_current: model.is_current,
        change_reason: model.change_reason,
        created_by_kind: model.created_by_kind,
        execution_id: model.execution_id,
        created_at: model.created_at,
        reviewed_at: model.reviewed_at,
    }
}
