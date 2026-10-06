use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{NewStockGroup, StockGroup, StockGroupMembership};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupAxis {
    pub id: Uuid,
    pub key: String,
    pub sync_source: Option<String>,
}

#[derive(Debug, Error)]
pub enum StockGroupRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait StockGroupRepository: Send + Sync {
    async fn find_axis_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis_key: &str,
    ) -> Result<Option<GroupAxis>, StockGroupRepositoryError>;

    async fn find_group(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis_id: Uuid,
        axis_key: &str,
        group_key: &str,
    ) -> Result<Option<StockGroup>, StockGroupRepositoryError>;

    async fn find_sync_source_codes(
        &self,
        transaction: &UnitOfWorkTransaction,
        sync_source: &str,
        group_key: &str,
    ) -> Result<Vec<Option<String>>, StockGroupRepositoryError>;

    async fn insert_group(
        &self,
        transaction: &UnitOfWorkTransaction,
        group: NewStockGroup,
    ) -> Result<StockGroup, StockGroupRepositoryError>;

    async fn update_group(
        &self,
        transaction: &UnitOfWorkTransaction,
        group: StockGroup,
    ) -> Result<StockGroup, StockGroupRepositoryError>;

    async fn stock_exists(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError>;

    async fn list_stock_ids(
        &self,
        transaction: &UnitOfWorkTransaction,
        group_id: Uuid,
    ) -> Result<Vec<String>, StockGroupRepositoryError>;

    async fn list_memberships(
        &self,
        transaction: &UnitOfWorkTransaction,
        stock_ids: &[String],
        axis_keys: Option<&[String]>,
    ) -> Result<Vec<StockGroupMembership>, StockGroupRepositoryError>;

    async fn add_stock(
        &self,
        transaction: &UnitOfWorkTransaction,
        group_id: Uuid,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError>;

    async fn remove_stock(
        &self,
        transaction: &UnitOfWorkTransaction,
        group_id: Uuid,
        stock_id: &str,
    ) -> Result<bool, StockGroupRepositoryError>;
}

pub type SharedStockGroupRepository = std::sync::Arc<dyn StockGroupRepository + Send + Sync>;
