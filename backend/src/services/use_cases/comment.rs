use std::sync::Arc;

use core_application::comment::CommentUseCases;
use gateway_postgres::PostgresCommentRepository;

use super::UseCases;

impl UseCases {
    pub fn comments(&self) -> CommentUseCases {
        CommentUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresCommentRepository),
            self.change_history.clone(),
        )
    }
}
