use thiserror::Error;
use uuid::Uuid;

use crate::annotation::{AnnotationReadUseCaseError, AnnotationReadUseCases};
use crate::note::{NoteReadUseCaseError, NoteReadUseCases};
use crate::strategy_scope::StrategyScope;

use super::query::{CommentListQuery, CommentReadQueryError, SharedCommentReadQuery};
use super::types::Comment;

#[derive(Debug, Error)]
pub enum CommentReadUseCaseError {
    #[error(transparent)]
    Query(#[from] CommentReadQueryError),
    #[error(transparent)]
    AnnotationRead(#[from] AnnotationReadUseCaseError),
    #[error(transparent)]
    NoteRead(#[from] NoteReadUseCaseError),
}

#[derive(Clone)]
pub struct CommentReadUseCases {
    query: SharedCommentReadQuery,
    annotation_reads: AnnotationReadUseCases,
    note_reads: NoteReadUseCases,
}

impl CommentReadUseCases {
    pub fn new(
        query: SharedCommentReadQuery,
        annotation_reads: AnnotationReadUseCases,
        note_reads: NoteReadUseCases,
    ) -> Self {
        Self {
            query,
            annotation_reads,
            note_reads,
        }
    }

    pub async fn list_comments(
        &self,
        query: CommentListQuery,
        _scope: Option<StrategyScope>,
    ) -> Result<Vec<Comment>, CommentReadUseCaseError> {
        match query.target_kind {
            super::types::CommentTargetKind::NoteVersion => {
                self.note_reads
                    .ensure_note_version_exists(query.target_id)
                    .await?;
            }
            super::types::CommentTargetKind::Annotation => {
                self.annotation_reads
                    .get_annotation(query.target_id, None)
                    .await?;
            }
        }
        Ok(self.query.list_comments(query).await?)
    }

    pub async fn count_line_comments(
        &self,
        note_version_id: Uuid,
    ) -> Result<u64, CommentReadUseCaseError> {
        Ok(self.query.count_line_comments(note_version_id).await?)
    }
}
