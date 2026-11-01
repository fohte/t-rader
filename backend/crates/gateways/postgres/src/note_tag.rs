use async_trait::async_trait;
use core_application::note_tag::{NewNoteTag, NoteTag, NoteTagRepository, NoteTagRepositoryError};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{Set, Unchanged};
use sea_orm::{ActiveModelTrait, EntityTrait, QueryOrder};

use crate::DatabaseHandle;
use crate::entities::note_tag;
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone)]
pub struct PostgresNoteTagRepository {
    db: DatabaseHandle,
}

impl PostgresNoteTagRepository {
    pub fn new(db: impl Into<DatabaseHandle>) -> Self {
        Self { db: db.into() }
    }
}

#[async_trait]
impl NoteTagRepository for PostgresNoteTagRepository {
    async fn list(&self) -> Result<Vec<NoteTag>, NoteTagRepositoryError> {
        note_tag::Entity::find()
            .order_by_asc(note_tag::Column::DisplayName)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_note_tag).collect())
            .map_err(repository_error)
    }

    async fn find_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<NoteTag>, NoteTagRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteTagRepositoryError::InvalidTransaction)?;
        note_tag::Entity::find_by_id(key.to_string())
            .one(transaction)
            .await
            .map(|row| row.map(to_note_tag))
            .map_err(repository_error)
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_tag: NewNoteTag,
    ) -> Result<NoteTag, NoteTagRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteTagRepositoryError::InvalidTransaction)?;
        note_tag::Entity::insert(note_tag::ActiveModel {
            key: Set(note_tag.key),
            display_name: Set(note_tag.display_name),
            description: Set(note_tag.description),
        })
        .exec_with_returning(transaction)
        .await
        .map(to_note_tag)
        .map_err(repository_error)
    }

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_tag: NoteTag,
    ) -> Result<NoteTag, NoteTagRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteTagRepositoryError::InvalidTransaction)?;
        note_tag::ActiveModel {
            key: Unchanged(note_tag.key),
            display_name: Set(note_tag.display_name),
            description: Set(note_tag.description),
        }
        .update(transaction)
        .await
        .map(to_note_tag)
        .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, NoteTagRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(NoteTagRepositoryError::InvalidTransaction)?;
        note_tag::Entity::delete_by_id(key.to_string())
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> NoteTagRepositoryError {
    NoteTagRepositoryError::Database(persistence_error(error))
}

fn to_note_tag(model: note_tag::Model) -> NoteTag {
    NoteTag {
        key: model.key,
        display_name: model.display_name,
        description: model.description,
    }
}
