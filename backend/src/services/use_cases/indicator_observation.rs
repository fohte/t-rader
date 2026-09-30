use std::sync::Arc;

use core_application::indicator_observation::IndicatorObservationUseCases;
use gateway_postgres::PostgresIndicatorObservationRepository;

use super::UseCases;

impl UseCases {
    pub fn indicator_observations(&self) -> IndicatorObservationUseCases {
        IndicatorObservationUseCases::new(Arc::new(PostgresIndicatorObservationRepository::new(
            self.db.clone(),
        )))
    }
}
