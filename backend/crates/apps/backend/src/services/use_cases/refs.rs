use std::sync::Arc;

use core_application::refs::RefUseCases;
use gateway_postgres::PostgresRefRepository;

use super::UseCases;

impl UseCases {
    pub fn refs(&self) -> RefUseCases {
        RefUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresRefRepository::new(self.db.clone())),
        )
    }
}
