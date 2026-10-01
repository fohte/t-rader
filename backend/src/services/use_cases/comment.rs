use std::sync::Arc;

use core_application::comment::{CommentReadUseCases, CommentUseCases};
use gateway_postgres::{PostgresCommentReadQuery, PostgresCommentRepository};

use super::UseCases;

impl UseCases {
    pub fn comment_reads(&self) -> CommentReadUseCases {
        CommentReadUseCases::new(
            Arc::new(PostgresCommentReadQuery::new(self.db.clone())),
            self.annotation_reads(),
            self.note_reads(),
        )
    }

    pub fn comments(&self) -> CommentUseCases {
        CommentUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresCommentRepository),
            self.change_history.clone(),
        )
    }
}
