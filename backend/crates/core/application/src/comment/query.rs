use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;

use super::types::{Comment, CommentTargetKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentListTarget {
    Target {
        target_kind: CommentTargetKind,
        target_id: Uuid,
    },
    AllNoteVersions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentListQuery {
    pub target: CommentListTarget,
    pub resolved: Option<bool>,
}

#[derive(Debug, Error)]
pub enum CommentReadQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait CommentReadQuery: Send + Sync {
    async fn list_comments(
        &self,
        query: CommentListQuery,
    ) -> Result<Vec<Comment>, CommentReadQueryError>;

    async fn count_line_comments(
        &self,
        note_version_id: Uuid,
    ) -> Result<u64, CommentReadQueryError>;
}

pub type SharedCommentReadQuery = Arc<dyn CommentReadQuery + Send + Sync>;
