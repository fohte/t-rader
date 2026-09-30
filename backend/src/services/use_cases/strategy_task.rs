use std::sync::Arc;

use core_application::strategy_task::StrategyTaskUseCases;
use gateway_postgres::PostgresStrategyTaskRepository;

use super::UseCases;

impl UseCases {
    pub fn strategy_tasks(&self) -> StrategyTaskUseCases {
        StrategyTaskUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStrategyTaskRepository::new(self.db.clone())),
        )
    }
}
