use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::error::CommentRepositoryError;
use super::repository::CommentRepository;
use super::types::{Comment, CommentTargetKind, NewComment, NoteVersionAnchorBodies};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FakeCommentTransaction {
    pub id: Uuid,
}

#[derive(Default)]
pub struct FakeCommentRepository {
    comments: Mutex<HashMap<Uuid, Comment>>,
    target_strategy_ids: Mutex<HashMap<(CommentTargetKind, Uuid), Uuid>>,
    note_version_bodies: Mutex<HashMap<Uuid, NoteVersionAnchorBodies>>,
    transaction_ids: Mutex<Vec<Uuid>>,
    update_count: Mutex<usize>,
}

impl FakeCommentRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_existing(&self, comment: Comment) {
        lock(&self.comments).insert(comment.id, comment);
    }

    pub fn set_target_strategy_id(
        &self,
        target_kind: CommentTargetKind,
        target_id: Uuid,
        strategy_id: Uuid,
    ) {
        lock(&self.target_strategy_ids).insert((target_kind, target_id), strategy_id);
    }

    pub fn set_note_version_anchor_bodies(
        &self,
        note_version_id: Uuid,
        bodies: NoteVersionAnchorBodies,
    ) {
        lock(&self.note_version_bodies).insert(note_version_id, bodies);
    }

    pub fn comments(&self) -> Vec<Comment> {
        lock(&self.comments).values().cloned().collect()
    }

    pub fn transaction_ids(&self) -> Vec<Uuid> {
        lock(&self.transaction_ids).clone()
    }

    pub fn update_count(&self) -> usize {
        *lock(&self.update_count)
    }
}

#[async_trait]
impl CommentRepository for FakeCommentRepository {
    async fn find_by_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Comment>, CommentRepositoryError> {
        self.record_transaction(transaction)?;
        Ok(lock(&self.comments).get(&id).cloned())
    }

    async fn target_strategy_id(
        &self,
        transaction: &UnitOfWorkTransaction,
        target_kind: CommentTargetKind,
        target_id: Uuid,
    ) -> Result<Option<Uuid>, CommentRepositoryError> {
        self.record_transaction(transaction)?;
        Ok(lock(&self.target_strategy_ids)
            .get(&(target_kind, target_id))
            .copied())
    }

    async fn note_version_anchor_bodies(
        &self,
        transaction: &UnitOfWorkTransaction,
        note_version_id: Uuid,
    ) -> Result<Option<NoteVersionAnchorBodies>, CommentRepositoryError> {
        self.record_transaction(transaction)?;
        Ok(lock(&self.note_version_bodies)
            .get(&note_version_id)
            .cloned())
    }

    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        comment: NewComment,
    ) -> Result<Comment, CommentRepositoryError> {
        self.record_transaction(transaction)?;
        let created_at = DateTime::<Utc>::UNIX_EPOCH.fixed_offset();
        let created = Comment {
            id: comment.id,
            target_kind: comment.target_kind.as_str().to_string(),
            target_id: comment.target_id,
            parent_id: comment.parent_id,
            body: comment.body,
            author_kind: comment.author_kind,
            author_label: comment.author_label,
            resolved: false,
            created_at,
            anchor_text: comment.anchor_text,
            anchor_side: comment.anchor_side,
            start_line: comment.start_line,
            end_line: comment.end_line,
        };
        lock(&self.comments).insert(created.id, created.clone());
        Ok(created)
    }

    async fn update_resolved(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        resolved: bool,
    ) -> Result<Option<Comment>, CommentRepositoryError> {
        self.record_transaction(transaction)?;
        *lock(&self.update_count) += 1;
        let mut comments = lock(&self.comments);
        let Some(comment) = comments.get_mut(&id) else {
            return Ok(None);
        };
        comment.resolved = resolved;
        Ok(Some(comment.clone()))
    }

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, CommentRepositoryError> {
        self.record_transaction(transaction)?;
        Ok(lock(&self.comments).remove(&id).is_some())
    }
}

impl FakeCommentRepository {
    fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), CommentRepositoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeCommentTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(CommentRepositoryError::InvalidTransaction)?;
        lock(&self.transaction_ids).push(transaction_id);
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
