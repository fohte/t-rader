use thiserror::Error;

use crate::daily_bar_source::DailyBarSourceError;
use crate::market_daily_bar_source::MarketDailyBarSourceError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::BarsRepositoryError;

#[derive(Debug, Error)]
pub enum BarsUseCaseError {
    #[error(transparent)]
    Repository(#[from] BarsRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    DailyBarSource(#[from] DailyBarSourceError),
    #[error(transparent)]
    MarketDailyBarSource(#[from] MarketDailyBarSourceError),
}
