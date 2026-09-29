use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{InvestableAmount, NewInvestableAmount, NewStrategy, Strategy};

#[derive(Debug, Error)]
pub enum StrategyRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait StrategyRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Strategy>, StrategyRepositoryError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Strategy>, StrategyRepositoryError>;
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Strategy>, StrategyRepositoryError>;
    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy: NewStrategy,
    ) -> Result<Strategy, StrategyRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy: Strategy,
    ) -> Result<Strategy, StrategyRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, StrategyRepositoryError>;
    async fn delete_confirmed(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        expected_name: &str,
    ) -> Result<bool, StrategyRepositoryError>;
    async fn find_current_investable_amount(
        &self,
        strategy_id: Uuid,
        as_of: chrono::DateTime<chrono::FixedOffset>,
    ) -> Result<Option<InvestableAmount>, StrategyRepositoryError>;
    async fn record_investable_amount(
        &self,
        transaction: &UnitOfWorkTransaction,
        amount: NewInvestableAmount,
    ) -> Result<InvestableAmount, StrategyRepositoryError>;
}

pub type SharedStrategyRepository = std::sync::Arc<dyn StrategyRepository + Send + Sync>;
