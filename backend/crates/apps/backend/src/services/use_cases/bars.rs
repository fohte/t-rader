use std::sync::Arc;

use core_application::bars::{BarsUseCases, SharedUsStockBarsWriteLock};
use gateway_postgres::PostgresBarsRepository;

use super::UseCases;

impl UseCases {
    pub fn bars(&self) -> BarsUseCases {
        BarsUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
        )
    }

    pub fn bars_with_write_lock(
        &self,
        write_lock: SharedUsStockBarsWriteLock,
    ) -> BarsUseCases {
        BarsUseCases::new_with_us_stock_bars_write_lock(
            self.unit_of_work.clone(),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
            write_lock,
        )
    }
}
