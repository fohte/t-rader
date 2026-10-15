use chrono::NaiveDate;
use thiserror::Error;

use crate::bars::BarsRepositoryError;

use super::repository::ValuationRepositoryError;

#[derive(Debug, Error)]
pub enum ValuationUseCaseError {
    #[error("東証営業日の valuation が未公開です: {0}")]
    MissingBusinessDayData(NaiveDate),
    #[error("東証営業日の valuation 取り込みに失敗しました: {0}")]
    BusinessDayIngestFailed(NaiveDate),
    #[error(transparent)]
    Repository(#[from] ValuationRepositoryError),
    #[error(transparent)]
    BarsRepository(#[from] BarsRepositoryError),
}
