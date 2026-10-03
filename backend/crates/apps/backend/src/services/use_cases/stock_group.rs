use core_application::stock_group::StockGroupUseCases;
use gateway_postgres::PostgresStockGroupRepository;
use std::sync::Arc;

use super::UseCases;

impl UseCases {
    pub fn stock_groups(&self) -> StockGroupUseCases {
        StockGroupUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStockGroupRepository::new()),
            self.change_history.clone(),
        )
    }
}
