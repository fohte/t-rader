use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{
    NewNote, NewNoteLink, NewNoteVersion, Note, NoteLinkTarget, NoteMetadataUpdate, NoteVersion,
    NoteVersionUpdate,
};

#[derive(Debug, Error)]
pub enum NoteRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("persisted note data is invalid: {0}")]
    InvalidData(String),
}

#[async_trait]
pub trait NoteRepository: Send + Sync {
    async fn find_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<Note>, NoteRepositoryError>;
    async fn find_notes_by_ids(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_ids: &[Uuid],
    ) -> Result<Vec<Note>, NoteRepositoryError>;
    async fn find_note_by_execution_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        execution_id: &str,
    ) -> Result<Option<Note>, NoteRepositoryError>;
    async fn find_note_kind_requires_approval(
        &self,
        transaction: &UnitOfWorkTransaction,
        kind: &str,
    ) -> Result<Option<bool>, NoteRepositoryError>;
    async fn find_current_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError>;
    async fn find_versions_by_ids(
        &self,
        transaction: &UnitOfWorkTransaction,
        version_ids: &[Uuid],
    ) -> Result<Vec<NoteVersion>, NoteRepositoryError>;
    async fn find_latest_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError>;
    async fn find_latest_pending_versions_by_kind(
        &self,
        transaction: &UnitOfWorkTransaction,
        kind: &str,
    ) -> Result<Vec<NoteVersion>, NoteRepositoryError>;
    async fn find_version_by_number(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<Option<NoteVersion>, NoteRepositoryError>;
    async fn supersede_pending_versions_before(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        version_no: i32,
    ) -> Result<Vec<Uuid>, NoteRepositoryError>;
    async fn find_initial_created_by_kind(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<Option<String>, NoteRepositoryError>;
    async fn find_initial_created_by_kind_by_note_ids(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, String>, NoteRepositoryError>;
    async fn insert_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note: NewNote,
    ) -> Result<Option<Note>, NoteRepositoryError>;
    async fn update_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        update: NoteMetadataUpdate,
    ) -> Result<Note, NoteRepositoryError>;
    async fn delete_note(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
    ) -> Result<bool, NoteRepositoryError>;
    async fn insert_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        version: NewNoteVersion,
    ) -> Result<NoteVersion, NoteRepositoryError>;
    async fn update_version(
        &self,
        transaction: &UnitOfWorkTransaction,
        update: NoteVersionUpdate,
    ) -> Result<NoteVersion, NoteRepositoryError>;
    async fn update_note_timestamp(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        updated_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<(), NoteRepositoryError>;
    async fn replace_references(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_id: Uuid,
        references: Vec<(String, String)>,
    ) -> Result<(), NoteRepositoryError>;
    async fn find_note_link_targets(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_ids: &[Uuid],
    ) -> Result<Vec<NoteLinkTarget>, NoteRepositoryError>;
    async fn insert_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        links: Vec<NewNoteLink>,
    ) -> Result<(), NoteRepositoryError>;
    async fn copy_note_links(
        &self,
        transaction: &UnitOfWorkTransaction,
        from_version_id: Uuid,
        to_version_id: Uuid,
    ) -> Result<(), NoteRepositoryError>;
}

pub type SharedNoteRepository = std::sync::Arc<dyn NoteRepository + Send + Sync>;
