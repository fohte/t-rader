use std::sync::Arc;

use crate::{agent_task_client::SharedAgentTaskClient, event_dispatch::EventDispatchUseCases};
use async_trait::async_trait;

pub const EVENT_DISPATCH_JOB_IDENTIFIER: &str = "event_dispatch";
pub const EVENT_DISPATCH_QUEUE_NAME: &str = "event_dispatch";

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("failed to dispatch pending events: {message}")]
pub struct EventDispatchJobError {
    message: String,
}

impl EventDispatchJobError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[async_trait]
pub trait EventDispatchJobExecutor: Send + Sync {
    async fn dispatch_pending_events(&self) -> Result<(), EventDispatchJobError>;
}

pub type SharedEventDispatchJobExecutor = Arc<dyn EventDispatchJobExecutor + Send + Sync>;

#[derive(Clone)]
pub struct EventDispatchJobUseCases {
    executor: SharedEventDispatchJobExecutor,
}

impl EventDispatchJobUseCases {
    pub fn new(executor: SharedEventDispatchJobExecutor) -> Self {
        Self { executor }
    }

    pub async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobError> {
        self.executor.dispatch_pending_events().await
    }
}

pub struct InlineEventDispatchJobExecutor {
    event_dispatch: EventDispatchUseCases,
    agent_task_client: SharedAgentTaskClient,
}

impl InlineEventDispatchJobExecutor {
    pub fn new(
        event_dispatch: EventDispatchUseCases,
        agent_task_client: SharedAgentTaskClient,
    ) -> Self {
        Self {
            event_dispatch,
            agent_task_client,
        }
    }
}

#[async_trait]
impl EventDispatchJobExecutor for InlineEventDispatchJobExecutor {
    async fn dispatch_pending_events(&self) -> Result<(), EventDispatchJobError> {
        self.event_dispatch
            .dispatch_pending_events(self.agent_task_client.as_ref())
            .await
            .map(|_| ())
            .map_err(|error| EventDispatchJobError::new(error.to_string()))
    }
}
