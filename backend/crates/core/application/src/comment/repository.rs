use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::CommentRepositoryError;
use super::types::{Comment, CommentTargetKind, NewComment, NoteVersionAnchorBodies};

#[async_trait]
pub trait CommentRepository: Send + Sync {
    async fn find_by_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Comment>, CommentRepositoryError>;

    async fn target_strategy_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        target_kind: CommentTargetKind,
        target_id: Uuid,
    ) -> Result<Option<Option<Uuid>>, CommentRepositoryError>;

    async fn note_version_anchor_bodies(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_version_id: Uuid,
    ) -> Result<Option<NoteVersionAnchorBodies>, CommentRepositoryError>;

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        comment: NewComment,
    ) -> Result<Comment, CommentRepositoryError>;

    async fn update_resolved(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        resolved: bool,
    ) -> Result<Option<Comment>, CommentRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, CommentRepositoryError>;
}

pub type SharedCommentRepository = Arc<dyn CommentRepository + Send + Sync>;
