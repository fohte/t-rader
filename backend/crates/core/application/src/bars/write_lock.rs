use std::time::Duration;

use async_trait::async_trait;

use crate::persistence::PersistenceError;

#[async_trait]
pub trait UsStockBarsWriteLock: Send + Sync {
    async fn acquire(
        &self,
        wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError>;
}

pub trait UsStockBarsWriteLockGuard: Send {}

pub type SharedUsStockBarsWriteLock = std::sync::Arc<dyn UsStockBarsWriteLock>;

#[cfg(test)]
pub(crate) fn test_write_lock() -> SharedUsStockBarsWriteLock {
    std::sync::Arc::new(TestUsStockBarsWriteLock)
}

#[cfg(test)]
struct TestUsStockBarsWriteLock;

#[cfg(test)]
struct TestUsStockBarsWriteLockGuard;

#[cfg(test)]
impl UsStockBarsWriteLockGuard for TestUsStockBarsWriteLockGuard {}

#[cfg(test)]
#[async_trait]
impl UsStockBarsWriteLock for TestUsStockBarsWriteLock {
    async fn acquire(
        &self,
        _wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        Ok(Some(Box::new(TestUsStockBarsWriteLockGuard)))
    }
}
