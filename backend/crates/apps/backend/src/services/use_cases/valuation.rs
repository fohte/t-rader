use std::sync::Arc;

use core_application::valuation::ValuationUseCases;
use gateway_postgres::PostgresValuationRepository;

use super::UseCases;

impl UseCases {
    pub fn valuations(&self) -> ValuationUseCases {
        ValuationUseCases::new(Arc::new(PostgresValuationRepository::new(self.db.clone())))
    }
}
