use std::sync::Arc;

use core_application::trigger::TriggerUseCases;
use gateway_postgres::{PostgresStrategyRepository, PostgresTriggerRepository};

use super::UseCases;

impl UseCases {
    pub fn triggers(&self) -> TriggerUseCases {
        self.triggers_with_strategy_tasks(self.strategy_tasks())
    }

    pub(super) fn triggers_with_strategy_tasks(
        &self,
        strategy_tasks: core_application::strategy_task::StrategyTaskUseCases,
    ) -> TriggerUseCases {
        TriggerUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresTriggerRepository::new(self.db.clone())),
            self.strategy_existence.clone(),
            Arc::new(PostgresStrategyRepository::new(self.db.clone())),
            strategy_tasks,
        )
    }
}
