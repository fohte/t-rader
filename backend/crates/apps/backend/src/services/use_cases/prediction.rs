use std::sync::Arc;

use core_application::prediction::PredictionUseCases;
use gateway_postgres::PostgresPredictionRepository;

use super::UseCases;

impl UseCases {
    pub fn predictions(&self) -> PredictionUseCases {
        PredictionUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresPredictionRepository::new(self.db.clone())),
        )
    }
}
