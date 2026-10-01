use std::sync::Arc;

use core_application::bars::BarsUseCases;
use gateway_postgres::PostgresBarsRepository;

use super::UseCases;

impl UseCases {
    pub fn bars(&self) -> BarsUseCases {
        BarsUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresBarsRepository::new(self.db.clone())),
        )
    }
}
