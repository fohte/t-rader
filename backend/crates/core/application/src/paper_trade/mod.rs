mod error;
mod filling;
mod ledger;
mod portfolio;
mod repository;
mod stats;
mod types;
mod use_cases;

#[cfg(all(test, feature = "test-support"))]
mod test_support;

pub use error::PaperTradeUseCaseError;
pub use repository::{PaperTradeRepository, PaperTradeRepositoryError, SharedPaperTradeRepository};
pub use types::{
    NewPaperAccount, NewPaperOrder, PaperAccount, PaperOrder, PaperOrderOutcome,
    PaperOrderRejectReason, PaperOrderResult, PaperOrderSide, PaperOrderWithResult,
    PaperTradeAccountStats, PaperTradeFillStats, PaperTradePortfolio, PaperTradePosition,
};
pub use use_cases::PaperTradeUseCases;
