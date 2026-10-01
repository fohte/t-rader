mod error;
mod query;
mod read;
mod repository;
mod types;
mod use_cases;

pub use error::{CommentRepositoryError, CommentUseCaseError};
pub use query::{
    CommentListQuery, CommentReadQuery, CommentReadQueryError, SharedCommentReadQuery,
};
pub use read::{CommentReadUseCaseError, CommentReadUseCases};
pub use repository::{CommentRepository, SharedCommentRepository};
pub use types::{
    Comment, CommentTargetKind, CreateCommentCommand, DeleteCommentCommand, NewComment,
    NoteVersionAnchorBodies, ReplyCommentCommand, ResolveCommentCommand,
};
pub use use_cases::CommentUseCases;

#[cfg(any(test, feature = "test-support"))]
mod fake;

#[cfg(feature = "test-support")]
pub use fake::FakeCommentRepository;

#[cfg(test)]
mod tests;
