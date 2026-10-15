use std::sync::Arc;

use core_application::{
    event_dispatch::EventDispatchUseCases,
    event_dispatch_job::{EventDispatchJobUseCases, InlineEventDispatchJobExecutor},
    trigger::TriggerUseCases,
};
use gateway_postgres::PostgresEventRepository;

use super::UseCases;

impl UseCases {
    pub fn event_dispatch(&self) -> EventDispatchUseCases {
        self.event_dispatch_with_triggers(self.triggers_for_event_dispatch())
    }

    pub fn event_dispatch_job(&self) -> EventDispatchJobUseCases {
        let triggers = self.triggers_for_event_dispatch();
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

    fn triggers_for_event_dispatch(&self) -> TriggerUseCases {
        self.triggers_with_strategy_tasks(self.strategy_tasks_for_event_dispatch())
    }
}
