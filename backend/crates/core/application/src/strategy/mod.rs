mod error;
mod query;
mod repository;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::StrategyUseCaseError;
pub use query::{SharedStrategySummaryQuery, StrategySummaryQuery, StrategySummaryQueryError};
pub use repository::{SharedStrategyRepository, StrategyRepository, StrategyRepositoryError};
pub use types::{
    CreateStrategyCommand, InvestableAmount, NewInvestableAmount, NewStrategy, Strategy,
    StrategySummary, StrategyUpdateCommand,
};
pub use use_cases::StrategyUseCases;

#[cfg(feature = "test-support")]
pub use fake::{FakeStrategyRepository, FakeStrategySummaryQuery};
