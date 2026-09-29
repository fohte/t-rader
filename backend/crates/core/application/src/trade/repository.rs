use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{NewTrade, Trade, TradeListItem, TradeQuery, TradeUpdate};

#[derive(Debug, Error)]
pub enum TradeRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait TradeRepository: Send + Sync {
    async fn list(&self, query: TradeQuery) -> Result<Vec<TradeListItem>, TradeRepositoryError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Trade>, TradeRepositoryError>;
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<Trade>, TradeRepositoryError>;
    async fn strategy_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<bool, TradeRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade: NewTrade,
    ) -> Result<Trade, TradeRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        trade: TradeUpdate,
    ) -> Result<Trade, TradeRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, TradeRepositoryError>;
}

pub type SharedTradeRepository = std::sync::Arc<dyn TradeRepository + Send + Sync>;
