mod error;
#[cfg(any(test, feature = "test-support"))]
mod fake;
mod repository;
mod types;
mod use_cases;

#[cfg(test)]
mod tests;

pub use error::{IndicatorObservationRepositoryError, IndicatorObservationUseCaseError};
pub use repository::{IndicatorObservationRepository, SharedIndicatorObservationRepository};
pub use types::{
    IndicatorObservationIngestResult, IndicatorObservationIngestSeriesResult,
    IndicatorObservationMetadata, IndicatorObservationQuery, IndicatorObservationReadResult,
    IndicatorObservationSeriesDefinition,
};
pub use use_cases::{IndicatorObservationUseCases, TWSE_SERIES};

#[cfg(any(test, feature = "test-support"))]
pub use fake::FakeIndicatorObservationRepository;
