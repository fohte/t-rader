use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::{ChangeHistoryError, ChangeHistoryPort, ChangeHistoryRecord};

#[derive(Debug, Clone, PartialEq)]
pub struct FakeChangeHistoryEntry {
    pub transaction_id: Uuid,
    pub record: ChangeHistoryRecord,
}

#[derive(Default)]
pub struct FakeChangeHistory {
    pub entries: Mutex<Vec<FakeChangeHistoryEntry>>,
}

impl FakeChangeHistory {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ChangeHistoryPort for FakeChangeHistory {
    async fn record(
        &self,
        transaction: &UnitOfWorkTransaction,
        record: ChangeHistoryRecord,
    ) -> Result<(), ChangeHistoryError> {
        let transaction_id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(ChangeHistoryError::InvalidTransaction)?;
        self.entries.lock().await.push(FakeChangeHistoryEntry {
            transaction_id,
            record,
        });
        Ok(())
    }
}
