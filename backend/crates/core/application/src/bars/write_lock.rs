use std::time::Duration;

use async_trait::async_trait;

use crate::persistence::PersistenceError;

use std::sync::Arc;

#[async_trait]
pub trait UsStockBarsWriteLock: Send + Sync {
    /// 呼び出し側が待機期限を管理し、期限到達時に future を cancel する。
    /// `wait_timeout` が 0 の場合は、待たずに取得を試す。
    async fn acquire(
        &self,
        wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError>;
}

pub trait UsStockBarsWriteLockGuard: Send {}

pub type SharedUsStockBarsWriteLock = std::sync::Arc<dyn UsStockBarsWriteLock>;

#[derive(Clone)]
pub struct SharedSipRefetchLease(Arc<dyn UsStockBarsWriteLock>);

impl SharedSipRefetchLease {
    pub fn new(lease: Arc<dyn UsStockBarsWriteLock>) -> Self {
        Self(lease)
    }

    pub(crate) async fn acquire(
        &self,
        wait_timeout: Duration,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
        self.0.acquire(wait_timeout).await
    }
}
