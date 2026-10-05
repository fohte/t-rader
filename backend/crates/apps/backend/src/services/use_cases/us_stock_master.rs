use std::sync::Arc;

use core_application::us_stock_master::UsStockMasterUseCases;
use gateway_postgres::PostgresUsStockMasterRepository;

use super::UseCases;

impl UseCases {
    pub fn us_stock_master(&self) -> UsStockMasterUseCases {
        UsStockMasterUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresUsStockMasterRepository),
        )
    }
}
