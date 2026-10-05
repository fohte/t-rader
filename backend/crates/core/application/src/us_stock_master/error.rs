use thiserror::Error;

use crate::unit_of_work::UnitOfWorkError;
use crate::us_stock_master::repository::UsStockMasterRepositoryError;
use crate::us_stock_master_source::UsStockMasterSourceError;

#[derive(Debug, Error)]
pub enum UsStockMasterUseCaseError {
    #[error(transparent)]
    Repository(#[from] UsStockMasterRepositoryError),
    #[error(transparent)]
    Source(#[from] UsStockMasterSourceError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
