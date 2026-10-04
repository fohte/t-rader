use super::{repository::CalendarEventRepositoryError, source::CalendarEventSourceError};
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventUseCaseError {
    #[error(transparent)]
    Source(#[from] CalendarEventSourceError),
    #[error(transparent)]
    Repository(#[from] CalendarEventRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error("calendar event source returned an invalid date range")]
    InvalidDateRange,
    #[error("calendar event source {expected} returned an event for {actual}")]
    SourceMismatch { expected: String, actual: String },
}
