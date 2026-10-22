use std::sync::Arc;

use core_application::bars::{
    BarsUseCases, SharedSipRefetchLease, SharedUsStockBarsWriteLock, UsStockBarsIngestUseCases,
};
use gateway_postgres::PostgresBarsRepository;

use super::UseCases;

impl UseCases {
    pub fn bars(&self) -> BarsUseCases {
        BarsUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
        )
    }

    pub fn us_stock_bars_ingest(
        &self,
        write_lock: SharedUsStockBarsWriteLock,
        sip_refetch_lease: SharedSipRefetchLease,
    ) -> UsStockBarsIngestUseCases {
        UsStockBarsIngestUseCases::new(self.bars(), write_lock, sip_refetch_lease)
    }
}
