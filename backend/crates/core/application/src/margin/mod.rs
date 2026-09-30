mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(any(test, feature = "test-support"))]
mod fake;

pub use error::MarginUseCaseError;
pub use repository::{MarginRepository, MarginRepositoryError, SharedMarginRepository};
pub use types::{IngestStats, MarginQuery, MarginReadResult};
pub use use_cases::MarginUseCases;

#[cfg(any(test, feature = "test-support"))]
pub use fake::FakeMarginRepository;
