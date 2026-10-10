use std::sync::Arc;

use core_application::valuation::ValuationUseCases;
use gateway_postgres::{PostgresBarsRepository, PostgresValuationRepository};

use super::UseCases;

impl UseCases {
    pub fn valuations(&self) -> ValuationUseCases {
        ValuationUseCases::new(
            Arc::new(PostgresValuationRepository::new(self.db.clone())),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
        )
    }
}
