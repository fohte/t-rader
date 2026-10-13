use thiserror::Error;

use crate::bars::BarsRepositoryError;
use crate::strategy::StrategyRepositoryError;
use crate::trade::{TradeRepositoryError, TradeUseCaseError};

#[derive(Debug, Error)]
pub enum PortfolioHistoryUseCaseError {
    #[error("date range is invalid or exceeds the maximum length")]
    InvalidDateRange,
    #[error(transparent)]
    TradeRepository(#[from] TradeRepositoryError),
    #[error(transparent)]
    TradeCalculation(#[from] TradeUseCaseError),
    #[error(transparent)]
    BarsRepository(#[from] BarsRepositoryError),
    #[error(transparent)]
    StrategyRepository(#[from] StrategyRepositoryError),
    #[error("portfolio history calculation failed: {0}")]
    Calculation(String),
}
