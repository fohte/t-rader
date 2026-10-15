use std::sync::Arc;

use core_application::strategy_task::StrategyTaskUseCases;
use core_application::event_dispatch_job::EventDispatchJobUseCases;
use gateway_postgres::{
    PostgresEventDispatchJobQueue, PostgresEventRepository, PostgresStrategyTaskRepository,
};

use super::UseCases;

impl UseCases {
    pub fn strategy_tasks(&self) -> StrategyTaskUseCases {
        StrategyTaskUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStrategyTaskRepository::new(self.db.clone())),
        )
        .with_completion_events(
            Arc::new(PostgresEventRepository::new(self.db.clone())),
            EventDispatchJobUseCases::new(Arc::new(PostgresEventDispatchJobQueue::new(
                self.db.clone(),
            ))),
        )
    }
}
