use crate::persistence::PersistenceError;

use super::{SharedStrategyTaskReconcileLock, StrategyTaskReconcileLockGuard};

#[derive(Clone)]
pub struct StrategyTaskReconcileLockUseCases {
    lock: SharedStrategyTaskReconcileLock,
}

impl StrategyTaskReconcileLockUseCases {
    pub fn new(lock: SharedStrategyTaskReconcileLock) -> Self {
        Self { lock }
    }

    pub async fn try_acquire(
        &self,
    ) -> Result<Option<Box<dyn StrategyTaskReconcileLockGuard>>, PersistenceError> {
        self.lock.try_acquire().await
    }
}
