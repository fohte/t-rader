use async_trait::async_trait;
use core_application::comment::{
    Comment, CommentListQuery, CommentReadQuery, CommentReadQueryError, CommentTargetKind,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::DatabaseHandle;
use crate::comment::to_domain;
use crate::entities::comment;
use crate::persistence::persistence_error;

#[derive(Clone)]
pub struct PostgresCommentReadQuery {
    db: DatabaseHandle,
}

impl PostgresCommentReadQuery {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CommentReadQuery for PostgresCommentReadQuery {
    async fn list_comments(
        &self,
        query: CommentListQuery,
    ) -> Result<Vec<Comment>, CommentReadQueryError> {
        let mut select = comment::Entity::find()
            .filter(comment::Column::TargetKind.eq(query.target_kind.as_str()))
            .filter(comment::Column::TargetId.eq(query.target_id));
        if let Some(resolved) = query.resolved {
            select = select.filter(comment::Column::Resolved.eq(resolved));
        }
        select
            .order_by_asc(comment::Column::CreatedAt)
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(to_domain).collect())
            .map_err(query_error)
    }

    async fn count_line_comments(
        &self,
        note_version_id: Uuid,
    ) -> Result<u64, CommentReadQueryError> {
        comment::Entity::find()
            .filter(comment::Column::TargetKind.eq(CommentTargetKind::NoteVersion.as_str()))
            .filter(comment::Column::TargetId.eq(note_version_id))
            .filter(comment::Column::ParentId.is_null())
            .filter(comment::Column::StartLine.is_not_null())
            .count(&self.db)
            .await
            .map_err(query_error)
    }
}

fn query_error(error: sea_orm::DbErr) -> CommentReadQueryError {
    CommentReadQueryError::Database(persistence_error(error))
}
