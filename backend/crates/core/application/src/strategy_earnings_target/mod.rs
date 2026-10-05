mod error;
mod repository;
mod types;
mod use_cases;

pub use error::{StrategyEarningsTargetRepositoryError, StrategyEarningsTargetUseCaseError};
pub use repository::{SharedStrategyEarningsTargetRepository, StrategyEarningsTargetRepository};
pub use types::StrategyEarningsTarget;
pub use use_cases::StrategyEarningsTargetUseCases;
