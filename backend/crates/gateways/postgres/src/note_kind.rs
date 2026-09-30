use async_trait::async_trait;
use core_application::note_kind::{
    NewNoteKind, NoteKind, NoteKindRepository, NoteKindRepositoryError,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{Set, Unchanged};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::{note, note_kind};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone)]
pub struct PostgresNoteKindRepository {
    db: DatabaseHandle,
}

impl PostgresNoteKindRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl NoteKindRepository for PostgresNoteKindRepository {
    async fn list(&self) -> Result<Vec<NoteKind>, NoteKindRepositoryError> {
        note_kind::Entity::find()
            .order_by_asc(note_kind::Column::SortOrder)
            .order_by_asc(note_kind::Column::Key)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_note_kind).collect())
            .map_err(repository_error)
    }

    async fn find_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<NoteKind>, NoteKindRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteKindRepositoryError::InvalidTransaction)?;
        note_kind::Entity::find_by_id(key.to_string())
            .one(transaction)
            .await
            .map(|row| row.map(to_note_kind))
            .map_err(repository_error)
    }

    async fn is_used_by_notes(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, NoteKindRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteKindRepositoryError::InvalidTransaction)?;
        note::Entity::find()
            .filter(note::Column::Kind.eq(key))
            .one(transaction)
            .await
            .map(|row| row.is_some())
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_kind: NewNoteKind,
    ) -> Result<NoteKind, NoteKindRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteKindRepositoryError::InvalidTransaction)?;
        note_kind::Entity::insert(note_kind::ActiveModel {
            key: Set(note_kind.key),
            display_name: Set(note_kind.display_name),
            requires_approval: Set(note_kind.requires_approval),
            description: Set(note_kind.description),
            sort_order: Set(note_kind.sort_order),
        })
        .exec_with_returning(transaction)
        .await
        .map(to_note_kind)
        .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_kind: NoteKind,
    ) -> Result<NoteKind, NoteKindRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteKindRepositoryError::InvalidTransaction)?;
        note_kind::ActiveModel {
            key: Unchanged(note_kind.key),
            display_name: Set(note_kind.display_name),
            requires_approval: Set(note_kind.requires_approval),
            description: Set(note_kind.description),
            sort_order: Set(note_kind.sort_order),
        }
        .update(transaction)
        .await
        .map(to_note_kind)
        .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, NoteKindRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteKindRepositoryError::InvalidTransaction)?;
        note_kind::Entity::delete_by_id(key.to_string())
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> NoteKindRepositoryError {
    NoteKindRepositoryError::Database(persistence_error(error))
}

fn to_note_kind(model: note_kind::Model) -> NoteKind {
    NoteKind {
        key: model.key,
        display_name: model.display_name,
        requires_approval: model.requires_approval,
        description: model.description,
        sort_order: model.sort_order,
    }
}
