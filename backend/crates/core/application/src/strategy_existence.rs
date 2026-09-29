use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Debug, Error)]
pub enum StrategyExistenceError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait StrategyExistence: Send + Sync {
    async fn exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<bool, StrategyExistenceError>;
}

pub type SharedStrategyExistence = Arc<dyn StrategyExistence + Send + Sync>;

#[cfg(feature = "test-support")]
mod fake {
    use std::collections::HashSet;

    use tokio::sync::Mutex;

    use super::*;
    use crate::unit_of_work::FakeTransaction;

    #[derive(Default)]
    pub struct FakeStrategyExistence {
        strategy_ids: Mutex<HashSet<Uuid>>,
        transaction_ids: Mutex<Vec<Uuid>>,
    }

    impl FakeStrategyExistence {
        pub fn new() -> Self {
            Self::default()
        }

        pub async fn insert_strategy(&self, strategy_id: Uuid) {
            self.strategy_ids.lock().await.insert(strategy_id);
        }

        pub async fn transaction_ids(&self) -> Vec<Uuid> {
            self.transaction_ids.lock().await.clone()
        }
    }

    #[async_trait]
    impl StrategyExistence for FakeStrategyExistence {
        async fn exists(
            &self,
            transaction: &UnitOfWorkTransaction,
            strategy_id: Uuid,
        ) -> Result<bool, StrategyExistenceError> {
            let transaction_id = transaction
                .downcast_ref::<FakeTransaction>()
                .map(|transaction| transaction.id)
                .ok_or(StrategyExistenceError::InvalidTransaction)?;
            self.transaction_ids.lock().await.push(transaction_id);
            Ok(self.strategy_ids.lock().await.contains(&strategy_id))
        }
    }
}

#[cfg(feature = "test-support")]
pub use fake::FakeStrategyExistence;
