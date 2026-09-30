use std::sync::Arc;

use core_application::custom_indicator::CustomIndicatorUseCases;
use gateway_postgres::PostgresCustomIndicatorRepository;

use super::UseCases;

impl UseCases {
    pub fn custom_indicators(&self) -> CustomIndicatorUseCases {
        CustomIndicatorUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresCustomIndicatorRepository::new(self.db.clone())),
            self.strategy_existence.clone(),
            self.change_history.clone(),
        )
    }
}
