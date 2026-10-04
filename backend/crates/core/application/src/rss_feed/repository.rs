use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{NewRssFeed, RssFeed, UpdateRssFeedPatch};

#[derive(Debug, Error)]
pub enum RssFeedRepositoryError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("rss feed with source '{0}' already exists")]
    DuplicateSource(String),
    #[error("rss feed has unsupported content source '{0}'")]
    InvalidContentSource(String),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait RssFeedRepository: Send + Sync {
    async fn list(&self, enabled_only: bool) -> Result<Vec<RssFeed>, RssFeedRepositoryError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<RssFeed>, RssFeedRepositoryError>;
    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<Option<RssFeed>, RssFeedRepositoryError>;
    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        feed: NewRssFeed,
    ) -> Result<RssFeed, RssFeedRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
        patch: UpdateRssFeedPatch,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<RssFeed, RssFeedRepositoryError>;
    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        id: Uuid,
    ) -> Result<bool, RssFeedRepositoryError>;
}

pub type SharedRssFeedRepository = std::sync::Arc<dyn RssFeedRepository + Send + Sync>;
