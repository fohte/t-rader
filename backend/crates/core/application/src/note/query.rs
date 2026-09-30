use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;

use super::types::{Note, NoteLink, NoteLinks, NoteListPage, NoteListQuery, NoteVersion};

#[derive(Debug, Error)]
pub enum NoteReadQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("persisted note data is invalid: {0}")]
    InvalidData(String),
}

#[async_trait]
pub trait NoteReadQuery: Send + Sync {
    async fn find_note(&self, note_id: Uuid) -> Result<Option<Note>, NoteReadQueryError>;
    async fn find_note_for_version(
        &self,
        version_id: Uuid,
    ) -> Result<Option<Note>, NoteReadQueryError>;
    async fn find_note_version(
        &self,
        note_id: Uuid,
        version_id: Option<Uuid>,
        use_latest_if_no_current: bool,
    ) -> Result<Option<NoteVersion>, NoteReadQueryError>;
    async fn find_initial_created_by_kind(
        &self,
        note_id: Uuid,
    ) -> Result<Option<String>, NoteReadQueryError>;
    async fn list_note_versions(
        &self,
        note_id: Uuid,
    ) -> Result<Vec<NoteVersion>, NoteReadQueryError>;
    async fn find_note_version_by_number(
        &self,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<Option<NoteVersion>, NoteReadQueryError>;
    async fn list_pending_note_versions(&self) -> Result<Vec<NoteVersion>, NoteReadQueryError>;
    async fn list_notes(&self, query: NoteListQuery) -> Result<NoteListPage, NoteReadQueryError>;
    async fn find_links_from_version(
        &self,
        version_id: Uuid,
    ) -> Result<Vec<NoteLink>, NoteReadQueryError>;
    async fn list_note_links(
        &self,
        note_id: Uuid,
        source_version_id: Uuid,
    ) -> Result<NoteLinks, NoteReadQueryError>;
}

pub type SharedNoteReadQuery = Arc<dyn NoteReadQuery + Send + Sync>;
