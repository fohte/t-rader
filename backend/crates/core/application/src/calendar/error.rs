use super::{repository::CalendarEventRepositoryError, source::CalendarEventSourceError};

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventUseCaseError {
    #[error(transparent)]
    Source(#[from] CalendarEventSourceError),
    #[error(transparent)]
    Repository(#[from] CalendarEventRepositoryError),
    #[error("calendar event source returned an invalid date range")]
    InvalidDateRange,
    #[error("calendar event source {expected} returned an event for {actual}")]
    SourceMismatch { expected: String, actual: String },
}
