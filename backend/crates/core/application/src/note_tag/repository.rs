use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{NewNoteTag, NoteTag};

#[derive(Debug, Error)]
pub enum NoteTagRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait NoteTagRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<NoteTag>, NoteTagRepositoryError>;
    async fn find_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<NoteTag>, NoteTagRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_tag: NewNoteTag,
    ) -> Result<NoteTag, NoteTagRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_tag: NoteTag,
    ) -> Result<NoteTag, NoteTagRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, NoteTagRepositoryError>;
}

pub type SharedNoteTagRepository = std::sync::Arc<dyn NoteTagRepository + Send + Sync>;
