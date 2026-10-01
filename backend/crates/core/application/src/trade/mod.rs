mod error;
mod import;
mod repository;
mod trade_notes;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::TradeUseCaseError;
pub use repository::{SharedTradeRepository, TradeRepository, TradeRepositoryError};
pub use trade_notes::TradeNoteUseCases;
pub use types::{
    CreateTradeCommand, NewTrade, NewTradeNoteLink, PerformanceSummary, PositionSummary,
    SbiImportResult, SbiImportRow, Trade, TradeListItem, TradeMatchQuery, TradeNoteLink,
    TradeNoteReference, TradeOrder, TradeQuery, TradeUpdate, TradeUpdateCommand,
};
pub use use_cases::TradeUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeTradeRepository;
