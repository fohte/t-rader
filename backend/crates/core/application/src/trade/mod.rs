mod error;
mod repository;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::TradeUseCaseError;
pub use repository::{SharedTradeRepository, TradeRepository, TradeRepositoryError};
pub use types::{
    CreateTradeCommand, NewTrade, PerformanceSummary, PositionSummary, Trade, TradeListItem,
    TradeNoteReference, TradeOrder, TradeQuery, TradeUpdate, TradeUpdateCommand,
};
pub use use_cases::TradeUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeTradeRepository;
