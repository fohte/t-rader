mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::StockGroupUseCaseError;
pub use repository::{
    GroupAxis, SharedStockGroupRepository, StockGroupRepository, StockGroupRepositoryError,
};
pub use types::{CreateStockGroupCommand, NewStockGroup, StockGroup, UpdateStockGroupCommand};
pub use use_cases::StockGroupUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeStockGroupRepository;

#[cfg(all(test, feature = "test-support"))]
mod tests;
