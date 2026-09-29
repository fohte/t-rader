use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{CustomIndicator, NewCustomIndicator};

#[derive(Debug, Error)]
pub enum CustomIndicatorRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait CustomIndicatorRepository: Send + Sync {
    async fn list_global(&self) -> Result<Vec<CustomIndicator>, CustomIndicatorRepositoryError>;
    async fn list_strategy(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
    ) -> Result<Vec<CustomIndicator>, CustomIndicatorRepositoryError>;
    async fn find_by_id(
        &self,
        indicator_id: Uuid,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError>;
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator_id: Uuid,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError>;
    async fn resolve_for_strategy(
        &self,
        strategy_id: Uuid,
        name: &str,
    ) -> Result<Option<CustomIndicator>, CustomIndicatorRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator: NewCustomIndicator,
    ) -> Result<CustomIndicator, CustomIndicatorRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator: CustomIndicator,
    ) -> Result<CustomIndicator, CustomIndicatorRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        indicator_id: Uuid,
    ) -> Result<bool, CustomIndicatorRepositoryError>;
}

pub type SharedCustomIndicatorRepository =
    std::sync::Arc<dyn CustomIndicatorRepository + Send + Sync>;
