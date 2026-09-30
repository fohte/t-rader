use async_trait::async_trait;
use core_application::comment::{
    Comment, CommentRepository, CommentRepositoryError, CommentTargetKind, NewComment,
    NoteVersionAnchorBodies,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};
use uuid::Uuid;

use crate::entities::{annotation, comment, note, note_version};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref;

#[derive(Clone, Copy, Default)]
pub struct PostgresCommentRepository;

#[async_trait]
impl CommentRepository for PostgresCommentRepository {
    async fn find_by_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Comment>, CommentRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(CommentRepositoryError::InvalidTransaction)?;
        comment::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map(|row| row.map(to_domain))
            .map_err(repository_error)
    }

    async fn target_strategy_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        target_kind: CommentTargetKind,
        target_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, CommentRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(CommentRepositoryError::InvalidTransaction)?;
        match target_kind {
            CommentTargetKind::NoteVersion => {
                let version = note_version::Entity::find_by_id(target_id)
                    .one(transaction)
                    .await
                    .map_err(repository_error)?;
                let Some(version) = version else {
                    return Ok(None);
                };
                note::Entity::find_by_id(version.note_id)
                    .one(transaction)
                    .await
                    .map(|row| row.map(|note| note.strategy_id))
                    .map_err(repository_error)
            }
            CommentTargetKind::Annotation => annotation::Entity::find_by_id(target_id)
                .one(transaction)
                .await
                .map(|row| row.map(|annotation| annotation.strategy_id))
                .map_err(repository_error),
        }
    }

    async fn note_version_anchor_bodies(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_version_id: Uuid,
    ) -> Result<Option<NoteVersionAnchorBodies>, CommentRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(CommentRepositoryError::InvalidTransaction)?;
        let version = note_version::Entity::find_by_id(note_version_id)
            .one(transaction)
            .await
            .map_err(repository_error)?;
        let Some(version) = version else {
            return Ok(None);
        };
        let previous_body = if version.version_no <= 1 {
            None
        } else {
            note_version::Entity::find()
                .filter(note_version::Column::NoteId.eq(version.note_id))
                .filter(note_version::Column::VersionNo.eq(version.version_no - 1))
                .one(transaction)
                .await
                .map_err(repository_error)?
                .map(|previous| previous.body_md)
        };
        Ok(Some(NoteVersionAnchorBodies {
            version_no: version.version_no,
            current_body: version.body_md,
            previous_body,
        }))
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        comment: NewComment,
    ) -> Result<Comment, CommentRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(CommentRepositoryError::InvalidTransaction)?;
        let model = comment::ActiveModel {
            id: Set(comment.id),
            target_kind: Set(comment.target_kind.as_str().to_string()),
            target_id: Set(comment.target_id),
            parent_id: Set(comment.parent_id),
            body: Set(comment.body),
            author_kind: Set(comment.author_kind),
            author_label: Set(comment.author_label),
            resolved: NotSet,
            created_at: NotSet,
            anchor_text: Set(comment.anchor_text),
            anchor_side: Set(comment.anchor_side),
            start_line: Set(comment.start_line),
            end_line: Set(comment.end_line),
        };
        comment::Entity::insert(model)
            .exec_with_returning(transaction)
            .await
            .map(to_domain)
            .map_err(repository_error)
    }

    async fn update_resolved(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        resolved: bool,
    ) -> Result<Option<Comment>, CommentRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(CommentRepositoryError::InvalidTransaction)?;
        let Some(current) = comment::Entity::find_by_id(id)
            .one(transaction)
            .await
            .map_err(repository_error)?
        else {
            return Ok(None);
        };
        let mut active = current.into_active_model();
        active.resolved = Set(resolved);
        active
            .update(transaction)
            .await
            .map(|updated| Some(to_domain(updated)))
            .map_err(repository_error)
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, CommentRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(CommentRepositoryError::InvalidTransaction)?;
        comment::Entity::delete_by_id(id)
            .exec(transaction)
            .await
            .map(|result| result.rows_affected > 0)
            .map_err(repository_error)
    }
}

fn repository_error(error: sea_orm::DbErr) -> CommentRepositoryError {
    CommentRepositoryError::Database(persistence_error(error))
}

fn to_domain(model: comment::Model) -> Comment {
    Comment {
        id: model.id,
        target_kind: model.target_kind,
        target_id: model.target_id,
        parent_id: model.parent_id,
        body: model.body,
        author_kind: model.author_kind,
        author_label: model.author_label,
        resolved: model.resolved,
        created_at: model.created_at,
        anchor_text: model.anchor_text,
        anchor_side: model.anchor_side,
        start_line: model.start_line,
        end_line: model.end_line,
    }
}
