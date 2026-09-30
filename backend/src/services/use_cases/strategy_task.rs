use std::sync::Arc;

use core_application::strategy_task::StrategyTaskUseCases;
use core_application::unit_of_work::SharedUnitOfWork;
use gateway_postgres::{PostgresStrategyTaskRepository, PostgresUnitOfWork};

use super::UseCases;

impl UseCases {
    pub fn strategy_tasks(&self) -> StrategyTaskUseCases {
        let unit_of_work: SharedUnitOfWork = Arc::new(PostgresUnitOfWork::new(self.db.clone()));
        StrategyTaskUseCases::new(
            unit_of_work,
            Arc::new(PostgresStrategyTaskRepository::new(self.db.clone())),
        )
    }
}
