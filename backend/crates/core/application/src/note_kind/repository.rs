use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{NewNoteKind, NoteKind};

#[derive(Debug, Error)]
pub enum NoteKindRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait NoteKindRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<NoteKind>, NoteKindRepositoryError>;
    async fn find_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<NoteKind>, NoteKindRepositoryError>;
    async fn is_used_by_notes(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, NoteKindRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_kind: NewNoteKind,
    ) -> Result<NoteKind, NoteKindRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_kind: NoteKind,
    ) -> Result<NoteKind, NoteKindRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, NoteKindRepositoryError>;
}

pub type SharedNoteKindRepository = std::sync::Arc<dyn NoteKindRepository + Send + Sync>;
