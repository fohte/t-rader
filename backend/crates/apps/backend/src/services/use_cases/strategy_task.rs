use std::sync::Arc;

use async_trait::async_trait;
use core_application::{
    event_dispatch_job::{EventDispatchJobError, EventDispatchJobExecutor, EventDispatchJobUseCases},
    strategy_task::StrategyTaskUseCases,
};
use gateway_postgres::{PostgresEventRepository, PostgresStrategyTaskRepository};

use super::UseCases;

impl UseCases {
    pub fn strategy_tasks(&self) -> StrategyTaskUseCases {
        self.strategy_tasks_without_completion_events()
            .with_completion_events(
                Arc::new(PostgresEventRepository::new(self.db.clone())),
                self.event_dispatch_job(),
            )
    }

    pub(super) fn strategy_tasks_for_event_dispatch(&self) -> StrategyTaskUseCases {
        self.strategy_tasks_without_completion_events()
            .with_completion_events(
                Arc::new(PostgresEventRepository::new(self.db.clone())),
                EventDispatchJobUseCases::new(Arc::new(DeferredEventDispatchJobExecutor)),
            )
    }

    fn strategy_tasks_without_completion_events(&self) -> StrategyTaskUseCases {
        StrategyTaskUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresStrategyTaskRepository::new(self.db.clone())),
        )
    }
}

struct DeferredEventDispatchJobExecutor;

#[async_trait]
impl EventDispatchJobExecutor for DeferredEventDispatchJobExecutor {
    async fn dispatch_pending_events(&self) -> Result<(), EventDispatchJobError> {
        // 現在の dispatch loop が、新たに作られた completion event も処理する。
        Ok(())
    }
}
