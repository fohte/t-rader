use serde::Serialize;

use crate::unit_of_work::SharedUnitOfWork;
use crate::us_stock_master::repository::SharedUsStockMasterRepository;
use crate::us_stock_master_source::UsStockMasterSource;

use super::error::UsStockMasterUseCaseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct UsStockMasterSyncStats {
    pub stocks_upserted: usize,
}

#[derive(Clone)]
pub struct UsStockMasterUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedUsStockMasterRepository,
}

impl UsStockMasterUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedUsStockMasterRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn sync(
        &self,
        source: &dyn UsStockMasterSource,
    ) -> Result<UsStockMasterSyncStats, UsStockMasterUseCaseError> {
        let entries = source.fetch_all_us_stocks().await?;
        if entries.is_empty() {
            return Ok(UsStockMasterSyncStats::default());
        }

        let transaction = self.unit_of_work.begin().await?;
        let stocks_upserted = self.repository.upsert(&transaction, &entries).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(UsStockMasterSyncStats { stocks_upserted })
    }
}
