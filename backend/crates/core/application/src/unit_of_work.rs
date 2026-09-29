use std::any::Any;

use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;

#[cfg(feature = "test-support")]
mod fake;

#[derive(Debug, Error)]
pub enum UnitOfWorkError {
    #[error(transparent)]
    Begin(PersistenceError),
    #[error(transparent)]
    Commit(PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

/// gateway 固有の transaction を application から隠すための opaque handle。
pub struct UnitOfWorkTransaction {
    inner: Box<dyn Any + Send + Sync>,
}

impl UnitOfWorkTransaction {
    pub fn new<T: Any + Send + Sync>(transaction: T) -> Self {
        Self {
            inner: Box::new(transaction),
        }
    }

    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.inner.downcast_ref()
    }

    pub fn downcast<T: Any + Send + Sync>(self) -> Result<T, Self> {
        self.inner
            .downcast::<T>()
            .map(|transaction| *transaction)
            .map_err(|inner| Self { inner })
    }
}

#[async_trait]
pub trait UnitOfWork: Send + Sync {
    async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError>;
    async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError>;
}

pub type SharedUnitOfWork = std::sync::Arc<dyn UnitOfWork + Send + Sync>;

#[cfg(feature = "test-support")]
pub use fake::{FakeTransaction, FakeUnitOfWork};
