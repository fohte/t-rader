use async_trait::async_trait;
use chrono::NaiveDate;
use thiserror::Error;

use crate::persistence::PersistenceError;
use core_domain::short_sale_report::ShortSaleReport;

use super::types::ShortSaleReportQuery;

#[derive(Debug, Error)]
pub enum ShortSaleReportRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait ShortSaleReportRepository: Send + Sync {
    async fn find_latest_disc_date(
        &self,
    ) -> Result<Option<NaiveDate>, ShortSaleReportRepositoryError>;
    async fn upsert(
        &self,
        reports: Vec<ShortSaleReport>,
    ) -> Result<(), ShortSaleReportRepositoryError>;
    async fn read(
        &self,
        query: ShortSaleReportQuery,
    ) -> Result<Vec<ShortSaleReport>, ShortSaleReportRepositoryError>;
}

pub type SharedShortSaleReportRepository =
    std::sync::Arc<dyn ShortSaleReportRepository + Send + Sync>;
