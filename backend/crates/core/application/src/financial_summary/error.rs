use thiserror::Error;

use super::repository::FinancialSummaryRepositoryError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum FinancialSummaryUseCaseError {
    #[error(transparent)]
    Repository(#[from] FinancialSummaryRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
