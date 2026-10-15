use std::sync::Arc;

use async_trait::async_trait;

use crate::persistence::PersistenceError;

#[async_trait]
pub trait StrategyTaskReconcileLock: Send + Sync {
    async fn try_acquire(
        &self,
    ) -> Result<Option<Box<dyn StrategyTaskReconcileLockGuard>>, PersistenceError>;
}

#[async_trait]
pub trait StrategyTaskReconcileLockGuard: Send {
    async fn release(self: Box<Self>) -> Result<(), PersistenceError>;
}

pub type SharedStrategyTaskReconcileLock = Arc<dyn StrategyTaskReconcileLock + Send + Sync>;
