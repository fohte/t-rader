use std::sync::Arc;

use core_application::{
    event_dispatch::EventDispatchUseCases,
    event_dispatch_job::{EventDispatchJobUseCases, InlineEventDispatchJobExecutor},
    strategy_task::StrategyTaskUseCases,
    trigger::TriggerUseCases,
};
use gateway_postgres::{PostgresEventRepository, PostgresStrategyRepository, PostgresTriggerRepository};

use super::UseCases;

impl UseCases {
    pub fn event_dispatch(&self) -> EventDispatchUseCases {
        self.event_dispatch_with_triggers(self.triggers())
    }

    pub fn event_dispatch_job(&self) -> EventDispatchJobUseCases {
        let triggers = self.triggers_for_inline_dispatch();
        EventDispatchJobUseCases::new(Arc::new(InlineEventDispatchJobExecutor::new(
            self.event_dispatch_with_triggers(triggers),
            self.agent_task_client.clone(),
        )))
    }

    fn event_dispatch_with_triggers(&self, triggers: TriggerUseCases) -> EventDispatchUseCases {
        EventDispatchUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresEventRepository::new(self.db.clone())),
            triggers,
        )
    }

    fn triggers_for_inline_dispatch(&self) -> TriggerUseCases {
        TriggerUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(PostgresTriggerRepository::new(self.db.clone())),
            self.strategy_existence.clone(),
            Arc::new(PostgresStrategyRepository::new(self.db.clone())),
            self.strategy_tasks_without_completion_events(),
        )
    }

    fn strategy_tasks_without_completion_events(&self) -> StrategyTaskUseCases {
        StrategyTaskUseCases::new(
            self.unit_of_work.clone(),
            Arc::new(gateway_postgres::PostgresStrategyTaskRepository::new(
                self.db.clone(),
            )),
        )
    }
}
