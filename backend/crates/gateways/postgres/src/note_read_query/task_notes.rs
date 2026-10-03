use std::collections::BTreeSet;

use core_application::note::{NoteReadQueryError, NoteSnapshot};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::{note, note_version, strategy_task_step};

use super::{PostgresNoteReadQuery, find_current_versions, query_error};

pub(super) async fn list_notes_written_by_task(
    query: &PostgresNoteReadQuery,
    task_id: Uuid,
) -> Result<Vec<NoteSnapshot>, NoteReadQueryError> {
    let execution_ids = strategy_task_step::Entity::find()
        .filter(strategy_task_step::Column::TaskId.eq(task_id))
        .all(&query.db)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|step| step.execution_step_id.to_string())
        .collect::<Vec<_>>();
    if execution_ids.is_empty() {
        return Ok(Vec::new());
    }

    let note_ids = note_version::Entity::find()
        .filter(note_version::Column::ExecutionId.is_in(execution_ids))
        .all(&query.db)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|version| version.note_id)
        .collect::<BTreeSet<_>>();
    if note_ids.is_empty() {
        return Ok(Vec::new());
    }

    let note_ids = note_ids.into_iter().collect::<Vec<_>>();
    let versions = find_current_versions(&query.db, &note_ids).await?;
    let current_note_ids = note_ids
        .into_iter()
        .filter(|note_id| versions.contains_key(note_id))
        .collect::<Vec<_>>();
    if current_note_ids.is_empty() {
        return Ok(Vec::new());
    }

    let notes = note::Entity::find()
        .filter(note::Column::Id.is_in(current_note_ids))
        .order_by_desc(note::Column::UpdatedAt)
        .order_by_desc(note::Column::Id)
        .all(&query.db)
        .await
        .map_err(query_error)?;

    query.snapshots(notes, versions).await
}
