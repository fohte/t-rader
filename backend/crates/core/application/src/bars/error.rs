use chrono::NaiveDate;
use thiserror::Error;

use crate::bars::us_stock_source::UsStockBarSourceError;
use crate::daily_bar_source::DailyBarSourceError;
use crate::market_daily_bar_source::MarketDailyBarSourceError;
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
