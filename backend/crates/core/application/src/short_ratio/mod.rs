mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(any(test, feature = "test-support"))]
mod fake;

pub use error::ShortRatioUseCaseError;
pub use repository::{SharedShortRatioRepository, ShortRatioRepository, ShortRatioRepositoryError};
pub use types::{ShortRatioIngestStats, ShortRatioQuery};
pub use use_cases::ShortRatioUseCases;

#[cfg(any(test, feature = "test-support"))]
pub use fake::FakeShortRatioRepository;

#[cfg(test)]
mod tests;
