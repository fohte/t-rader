use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("failed to enqueue strategy task reconciliation job: {message}")]
pub struct StrategyTaskReconcileJobQueueError {
    message: String,
}

impl StrategyTaskReconcileJobQueueError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[async_trait]
pub trait StrategyTaskReconcileJobQueue: Send + Sync {
    async fn enqueue_reconciliation(&self) -> Result<(), StrategyTaskReconcileJobQueueError>;
}

pub type SharedStrategyTaskReconcileJobQueue = Arc<dyn StrategyTaskReconcileJobQueue + Send + Sync>;
