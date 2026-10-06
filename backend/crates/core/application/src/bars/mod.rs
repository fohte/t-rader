mod error;
mod ingest;
mod market_price;
mod repository;
mod types;
mod us_stock_ingest;
mod us_stock_source;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::BarsUseCaseError;
pub use repository::{BarsRepository, BarsRepositoryError, SharedBarsRepository};
pub use types::{
    BarsByInstrumentsQuery, BarsQuery, IngestStats, LatestPrices, UsStockBarTarget,
    UsStockBarsIngestStats,
};
pub use us_stock_source::{
    SharedUsStockBarSource, UsStockBarPage, UsStockBarQuery, UsStockBarSource,
    UsStockBarSourceError,
};
pub use use_cases::BarsUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeBarsRepository;
