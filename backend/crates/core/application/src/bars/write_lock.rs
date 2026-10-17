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

pub(super) fn no_op_write_lock() -> SharedUsStockBarsWriteLock {
    std::sync::Arc::new(NoopUsStockBarsWriteLock)
}

struct NoopUsStockBarsWriteLock;

struct NoopUsStockBarsWriteLockGuard;

impl UsStockBarsWriteLockGuard for NoopUsStockBarsWriteLockGuard {}

#[async_trait]
impl UsStockBarsWriteLock for NoopUsStockBarsWriteLock {
    async fn acquire(
        &self,
        _wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        Ok(Some(Box::new(NoopUsStockBarsWriteLockGuard)))
    }
}
