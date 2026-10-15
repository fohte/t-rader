use std::sync::Arc;

use async_trait::async_trait;
use core_application::{
    agent_task_client::SharedAgentTaskClient,
    event_dispatch::EventDispatchUseCases,
    event_dispatch_job::{
        EventDispatchJobQueue, EventDispatchJobQueueError, EventDispatchJobUseCases,
    },
    trigger::TriggerUseCases,
};
use gateway_postgres::PostgresEventRepository;

use super::{BackgroundEventDispatchControl, UseCases};

impl UseCases {
    pub fn event_dispatch(&self) -> EventDispatchUseCases {
        self.event_dispatch_with_triggers(self.triggers_for_event_dispatch())
    }

    pub fn event_dispatch_job(&self) -> EventDispatchJobUseCases {
        let triggers = self.triggers_for_event_dispatch();
        EventDispatchJobUseCases::new(Arc::new(BackgroundEventDispatchJobQueue {
            event_dispatch: self.event_dispatch_with_triggers(triggers),
            agent_task_client: self.agent_task_client.clone(),
            control: self.event_dispatch_control.clone(),
        }))
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

struct BackgroundEventDispatchJobQueue {
    event_dispatch: EventDispatchUseCases,
    agent_task_client: SharedAgentTaskClient,
    control: Arc<BackgroundEventDispatchControl>,
}

#[async_trait]
impl EventDispatchJobQueue for BackgroundEventDispatchJobQueue {
    async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobQueueError> {
        let should_spawn = {
            let mut state = self.control.state.lock().await;
            state.pending = true;
            if state.running {
                false
            } else {
                state.running = true;
                true
            }
        };
        if !should_spawn {
            return Ok(());
        }

        let event_dispatch = self.event_dispatch.clone();
        let agent_task_client = self.agent_task_client.clone();
        let control = self.control.clone();
        tokio::spawn(async move {
            loop {
                let should_dispatch = {
                    let mut state = control.state.lock().await;
                    if state.pending {
                        state.pending = false;
                        true
                    } else {
                        state.running = false;
                        false
                    }
                };
                if !should_dispatch {
                    break;
                }

                if let Err(error) = event_dispatch
                    .dispatch_pending_events(agent_task_client.as_ref())
                    .await
                {
                    tracing::warn!(%error, "event dispatch failed; periodic dispatch will retry");
                }
            }
        });
        Ok(())
    }
}
