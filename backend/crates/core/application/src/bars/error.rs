use chrono::NaiveDate;
use thiserror::Error;

use crate::bars::us_stock_source::UsStockBarSourceError;
use crate::daily_bar_source::DailyBarSourceError;
use crate::market_daily_bar_source::MarketDailyBarSourceError;
use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkError;

use super::repository::BarsRepositoryError;

#[derive(Debug, Error)]
pub enum BarsUseCaseError {
    #[error("東証営業日の日足が未公開です: {0}")]
    MissingBusinessDayData(NaiveDate),
    #[error("東証営業日の日足取り込みに失敗しました: {0}")]
    BusinessDayIngestFailed(NaiveDate),
    #[error(transparent)]
    Repository(#[from] BarsRepositoryError),
    #[error("米国株 bars の書き込みロックを取得できませんでした: {0}")]
    UsStockBarsWriteLock(#[from] PersistenceError),
    #[error("米国株 bars の書き込みロックが取得期限までに解放されませんでした")]
    UsStockBarsWriteLockBusy,
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error("米国株バーの commit probe に必要な取り込み実行コンテキストがありません")]
    IngestRunContextRequired,
    #[error(transparent)]
    DailyBarSource(#[from] DailyBarSourceError),
    #[error(transparent)]
    MarketDailyBarSource(#[from] MarketDailyBarSourceError),
    #[error(transparent)]
    UsStockBarSource(#[from] UsStockBarSourceError),
}

#[derive(Debug)]
pub struct UsStockBarsIngestError {
    stats: Box<super::types::UsStockBarsIngestResult>,
    error: BarsUseCaseError,
}

impl UsStockBarsIngestError {
    pub(crate) fn new(
        stats: super::types::UsStockBarsIngestResult,
        error: BarsUseCaseError,
    ) -> Self {
        Self {
            stats: Box::new(stats),
            error,
        }
    }

    pub fn into_parts(self) -> (super::types::UsStockBarsIngestResult, BarsUseCaseError) {
        (*self.stats, self.error)
    }
}

impl std::fmt::Display for UsStockBarsIngestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for UsStockBarsIngestError {}
