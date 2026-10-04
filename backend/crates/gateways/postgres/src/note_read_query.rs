use std::collections::HashMap;

use async_trait::async_trait;
use core_application::note::{
    Note, NoteLink, NoteLinks, NoteListPage, NoteListQuery, NoteReadQuery, NoteReadQueryError,
    NoteSnapshot, NoteVersion,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::entities::{note, note_version};
use crate::persistence::persistence_error;

const PENDING_STATUS: &str = "unread";

mod links;
mod list;
mod task_notes;

#[derive(Clone)]
pub struct PostgresNoteReadQuery {
    db: DatabaseHandle,
}

impl PostgresNoteReadQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }

    async fn snapshots(
        &self,
        rows: Vec<note::Model>,
        versions: HashMap<Uuid, note_version::Model>,
    ) -> Result<Vec<NoteSnapshot>, NoteReadQueryError> {
        let note_ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let creators = find_initial_created_by_kind(&self.db, &note_ids).await?;
        rows.into_iter()
            .map(|row| {
                let version = versions.get(&row.id).cloned().ok_or_else(|| {
                    NoteReadQueryError::InvalidData(format!(
                        "note {} has no selected version",
                        row.id
                    ))
                })?;
                let created_by_kind = creators.get(&row.id).cloned().ok_or_else(|| {
                    NoteReadQueryError::InvalidData(format!(
                        "note {} has no initial version",
                        row.id
                    ))
                })?;
                Ok(NoteSnapshot {
                    note: to_note(row),
                    version: to_version(version),
                    created_by_kind,
                })
            })
            .collect()
    }
}

#[async_trait]
impl NoteReadQuery for PostgresNoteReadQuery {
    async fn find_note(&self, note_id: Uuid) -> Result<Option<Note>, NoteReadQueryError> {
        note::Entity::find_by_id(note_id)
            .one(&self.db)
            .await
            .map(|row| row.map(to_note))
            .map_err(query_error)
    }

    async fn find_note_for_version(
        &self,
        version_id: Uuid,
    ) -> Result<Option<Note>, NoteReadQueryError> {
        let Some(version) = note_version::Entity::find_by_id(version_id)
            .one(&self.db)
            .await
            .map_err(query_error)?
        else {
            return Ok(None);
        };
        self.find_note(version.note_id).await
    }

    async fn find_note_version(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
        use_latest_if_no_current: bool,
    ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
        let version = match version_id {
            Some(version_id) => note_version::Entity::find_by_id(version_id)
                .filter(note_version::Column::NoteId.eq(note_id))
                .one(&self.db)
                .await
                .map_err(query_error)?,
            None => {
                let current = find_current_version(&self.db, note_id).await?;
                if current.is_some() || !use_latest_if_no_current {
                    current
                } else {
                    find_latest_version(&self.db, note_id).await?
                }
            }
        };
        Ok(version.map(to_version))
    }

    async fn find_initial_created_by_kind(
        &self,
        note_id: Uuid,
    ) -> Result<Option<String>, NoteReadQueryError> {
        Ok(find_initial_created_by_kind(&self.db, &[note_id])
            .await?
            .remove(&note_id))
    }

    async fn list_note_versions(
        &self,
        note_id: Uuid,
    ) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .order_by_asc(note_version::Column::VersionNo)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_version).collect())
            .map_err(query_error)
    }

    async fn find_note_version_by_number(
        &self,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<Option<NoteVersion>, NoteReadQueryError> {
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(note_id))
            .filter(note_version::Column::VersionNo.eq(version_no))
            .one(&self.db)
            .await
            .map(|row| row.map(to_version))
            .map_err(query_error)
    }

    async fn list_pending_note_versions(&self) -> Result<Vec<NoteVersion>, NoteReadQueryError> {
        note_version::Entity::find()
            .filter(note_version::Column::Status.eq(PENDING_STATUS))
            .order_by_asc(note_version::Column::CreatedAt)
            .order_by_asc(note_version::Column::NoteId)
            .order_by_asc(note_version::Column::VersionNo)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_version).collect())
            .map_err(query_error)
    }

    async fn list_notes(&self, query: NoteListQuery) -> Result<NoteListPage, NoteReadQueryError> {
        self.list_query(query).await
    }

    async fn list_notes_written_by_task(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<core_application::note::NoteSnapshot>, NoteReadQueryError> {
        task_notes::list_notes_written_by_task(self, task_id).await
    }

    async fn find_links_from_version(
        &self,
        version_id: Uuid,
    ) -> Result<Vec<NoteLink>, NoteReadQueryError> {
        links::find_links_from_version(&self.db, version_id).await
    }

    async fn list_note_links(
        &self,
        note_id: Uuid,
        source_version_id: Uuid,
    ) -> Result<NoteLinks, NoteReadQueryError> {
        links::list_note_links(&self.db, note_id, source_version_id).await
    }
}

async fn find_current_version(
    db: &DatabaseHandle,
    note_id: Uuid,
) -> Result<Option<note_version::Model>, NoteReadQueryError> {
    note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .filter(note_version::Column::IsCurrent.eq(true))
        .one(db)
        .await
        .map_err(query_error)
}

async fn find_latest_version(
    db: &DatabaseHandle,
    note_id: Uuid,
) -> Result<Option<note_version::Model>, NoteReadQueryError> {
    note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .order_by_desc(note_version::Column::VersionNo)
        .one(db)
        .await
        .map_err(query_error)
}

async fn find_current_versions(
    db: &DatabaseHandle,
    note_ids: &[Uuid],
) -> Result<HashMap<Uuid, note_version::Model>, NoteReadQueryError> {
    if note_ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(note_version::Entity::find()
        .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
        .filter(note_version::Column::IsCurrent.eq(true))
        .all(db)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|version| (version.note_id, version))
        .collect())
}

async fn find_latest_versions(
    db: &DatabaseHandle,
    note_ids: &[Uuid],
) -> Result<HashMap<Uuid, note_version::Model>, NoteReadQueryError> {
    if note_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut versions = HashMap::new();
    for version in note_version::Entity::find()
        .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
        .order_by_desc(note_version::Column::VersionNo)
        .all(db)
        .await
        .map_err(query_error)?
    {
        versions.entry(version.note_id).or_insert(version);
    }
    Ok(versions)
}

async fn find_initial_created_by_kind(
    db: &DatabaseHandle,
    note_ids: &[Uuid],
) -> Result<HashMap<Uuid, String>, NoteReadQueryError> {
    if note_ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(note_version::Entity::find()
        .filter(note_version::Column::NoteId.is_in(note_ids.iter().copied()))
        .filter(note_version::Column::VersionNo.eq(1))
        .all(db)
        .await
        .map_err(query_error)?
        .into_iter()
        .map(|version| (version.note_id, version.created_by_kind))
        .collect())
}

fn to_note(model: note::Model) -> Note {
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

fn to_version(model: note_version::Model) -> NoteVersion {
    NoteVersion {
        id: model.id,
        note_id: model.note_id,
        version_no: model.version_no,
        title: model.title,
        body_md: model.body_md,
        frontmatter_json: model.frontmatter_json,
        graphs_json: model.graphs_json,
        status: model.status,
        is_current: model.is_current,
        change_reason: model.change_reason,
        created_by_kind: model.created_by_kind,
        execution_id: model.execution_id,
        created_at: model.created_at,
        reviewed_at: model.reviewed_at,
    }
}

fn query_error(error: sea_orm::DbErr) -> NoteReadQueryError {
    NoteReadQueryError::Database(persistence_error(error))
}
