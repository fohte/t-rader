use std::sync::Arc;

use core_application::strategy::StrategyUseCases;
use gateway_postgres::{PostgresStrategyRepository, PostgresStrategySummaryQuery};

use super::UseCases;

impl UseCases {
    pub fn strategies(&self) -> StrategyUseCases {
        StrategyUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStrategyRepository::new(self.db.clone())),
            Arc::new(PostgresStrategySummaryQuery::new(self.db.clone())),
            self.change_history.clone(),
        )
    }
}
