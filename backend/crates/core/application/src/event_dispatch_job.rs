use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("failed to enqueue event dispatch job: {message}")]
pub struct EventDispatchJobQueueError {
    message: String,
}

impl EventDispatchJobQueueError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[async_trait]
pub trait EventDispatchJobQueue: Send + Sync {
    async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobQueueError>;
}

pub type SharedEventDispatchJobQueue = Arc<dyn EventDispatchJobQueue + Send + Sync>;

#[derive(Clone)]
pub struct EventDispatchJobUseCases {
    queue: SharedEventDispatchJobQueue,
}

impl EventDispatchJobUseCases {
    pub fn new(queue: SharedEventDispatchJobQueue) -> Self {
        Self { queue }
    }

    pub async fn enqueue_dispatch(&self) -> Result<(), EventDispatchJobQueueError> {
        self.queue.enqueue_dispatch().await
    }
}
