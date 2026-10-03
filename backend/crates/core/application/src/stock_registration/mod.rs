mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::StockRegistrationUseCaseError;
pub use repository::{
    SharedStockRegistrationRepository, StockRegistrationRepository,
    StockRegistrationRepositoryError,
};
pub use types::{NewStockRegistration, RegisterStockCommand, RegisteredStock};
pub use use_cases::StockRegistrationUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeStockRegistrationRepository;
