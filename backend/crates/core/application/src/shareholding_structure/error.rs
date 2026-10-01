use thiserror::Error;

use crate::shareholding_structure::repository::ShareholdingStructureRepositoryError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum ShareholdingStructureUseCaseError {
    #[error(transparent)]
    Repository(#[from] ShareholdingStructureRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
