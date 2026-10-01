use thiserror::Error;

use crate::equity_master::repository::EquityMasterRepositoryError;
use crate::equity_master_source::EquityMasterSourceError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum EquityMasterUseCaseError {
    #[error(transparent)]
    Repository(#[from] EquityMasterRepositoryError),
    #[error(transparent)]
    Source(#[from] EquityMasterSourceError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
