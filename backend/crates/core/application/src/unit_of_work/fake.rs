use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction};

#[derive(Debug)]
pub struct FakeTransaction {
    pub id: Uuid,
}

#[derive(Default)]
pub struct FakeUnitOfWork {
    pub begun: Mutex<Vec<Uuid>>,
    pub committed: Mutex<Vec<Uuid>>,
}

impl FakeUnitOfWork {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl UnitOfWork for FakeUnitOfWork {
    async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
        let transaction = FakeTransaction { id: Uuid::new_v4() };
        self.begun.lock().await.push(transaction.id);
        Ok(UnitOfWorkTransaction::new(transaction))
    }

    async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
        let transaction = transaction
            .downcast::<FakeTransaction>()
            .map_err(|_| UnitOfWorkError::InvalidTransaction)?;
        self.committed.lock().await.push(transaction.id);
        Ok(())
    }
}
