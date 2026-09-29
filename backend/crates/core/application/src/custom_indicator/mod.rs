mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::CustomIndicatorUseCaseError;
pub use repository::{
    CustomIndicatorRepository, CustomIndicatorRepositoryError, SharedCustomIndicatorRepository,
};
pub use types::{
    CreateCustomIndicatorCommand, CustomIndicator, NewCustomIndicator, SCOPE_GLOBAL,
    SCOPE_STRATEGY, UpdateCustomIndicatorCommand,
};
pub use use_cases::CustomIndicatorUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeCustomIndicatorRepository;
