use thiserror::Error;

use super::repository::ShortSaleReportRepositoryError;

#[derive(Debug, Error)]
pub enum ShortSaleReportUseCaseError {
    #[error(transparent)]
    Repository(#[from] ShortSaleReportRepositoryError),
}
