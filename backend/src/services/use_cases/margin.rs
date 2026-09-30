use std::sync::Arc;

use core_application::margin::MarginUseCases;
use gateway_postgres::PostgresMarginRepository;

use super::UseCases;

impl UseCases {
    pub fn margins(&self) -> MarginUseCases {
        MarginUseCases::new(Arc::new(PostgresMarginRepository::new(self.db.clone())))
    }
}
