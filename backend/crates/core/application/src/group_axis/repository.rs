use async_trait::async_trait;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{GroupAxis, NewGroupAxis};

#[derive(Debug, Error)]
pub enum GroupAxisRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait GroupAxisRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<GroupAxis>, GroupAxisRepositoryError>;
    async fn find_by_key(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<Option<GroupAxis>, GroupAxisRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis: NewGroupAxis,
    ) -> Result<GroupAxis, GroupAxisRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        axis: GroupAxis,
    ) -> Result<GroupAxis, GroupAxisRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        key: &str,
    ) -> Result<bool, GroupAxisRepositoryError>;
}

pub type SharedGroupAxisRepository = std::sync::Arc<dyn GroupAxisRepository + Send + Sync>;
