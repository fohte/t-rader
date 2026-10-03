use core_application::stock_registration::StockRegistrationUseCases;
use gateway_postgres::PostgresStockRegistrationRepository;
use std::sync::Arc;

use super::UseCases;

impl UseCases {
    pub fn stock_registration(&self) -> StockRegistrationUseCases {
        StockRegistrationUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStockRegistrationRepository::new()),
            self.change_history.clone(),
        )
    }
}
