mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(any(test, feature = "test-support"))]
mod fake;

pub use error::ShortSaleReportUseCaseError;
pub use repository::{
    SharedShortSaleReportRepository, ShortSaleReportRepository, ShortSaleReportRepositoryError,
};
pub use types::{ShortSaleReportIngestStats, ShortSaleReportQuery};
pub use use_cases::ShortSaleReportUseCases;

#[cfg(any(test, feature = "test-support"))]
pub use fake::FakeShortSaleReportRepository;

#[cfg(test)]
mod tests;
