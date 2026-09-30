use thiserror::Error;

use super::repository::EarningsScheduleRepositoryError;

#[derive(Debug, Error)]
pub enum EarningsScheduleUseCaseError {
    #[error(transparent)]
    Repository(#[from] EarningsScheduleRepositoryError),
}
