mod error;
mod fifo;
mod import;
mod repository;
mod summary;
mod trade_notes;
mod types;
mod use_cases;

#[cfg(feature = "test-support")]
mod fake;

pub use error::TradeUseCaseError;
pub use repository::{SharedTradeRepository, TradeRepository, TradeRepositoryError};
pub use summary::{
    TradeSummaryUseCaseError, TradeSummaryUseCases, ValuedPerformanceSummary, ValuedPositionSummary,
};
pub use trade_notes::TradeNoteUseCases;
pub use types::{
    CreateTradeCommand, MatchedBuy, NewTrade, NewTradeNoteLink, PerformanceSummary,
    PositionSummary, SbiImportResult, SbiImportRow, Trade, TradeDetail, TradeListItem,
    TradeMatchQuery, TradeNoteLink, TradeNoteReference, TradeOrder, TradeQuery, TradeUpdate,
    TradeUpdateCommand,
};
pub use use_cases::TradeUseCases;

#[cfg(feature = "test-support")]
pub use fake::FakeTradeRepository;
