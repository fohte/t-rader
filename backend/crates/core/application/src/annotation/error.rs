use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AnnotationUseCaseError {
    #[error("{0}")]
    Validation(String),
    #[error("annotation {0} not found")]
    NotFound(Uuid),
    #[error("linked note {0} not found")]
    LinkedNoteNotFound(Uuid),
    #[error(transparent)]
    Repository(#[from] super::ports::AnnotationRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] crate::unit_of_work::UnitOfWorkError),
    #[error(transparent)]
    ChangeHistory(#[from] crate::change_history::ChangeHistoryError),
}
