use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{NewTrigger, Trigger, TriggerKind};

#[derive(Debug, Error)]
pub enum TriggerRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("invalid trigger kind: {0}")]
    InvalidKind(String),
}

#[async_trait]
pub trait TriggerRepository: Send + Sync {
    async fn list_for_strategy(
        &self,
        transaction: &UnitOfWorkTransaction,
        strategy_id: Uuid,
        kind: Option<TriggerKind>,
    ) -> Result<Vec<Trigger>, TriggerRepositoryError>;

    async fn find_by_id(&self, trigger_id: Uuid)
    -> Result<Option<Trigger>, TriggerRepositoryError>;

    async fn find_by_id_in_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
    ) -> Result<Option<Trigger>, TriggerRepositoryError>;

    async fn find_enabled_hook_by_slug(
        &self,
        hook_slug: &str,
    ) -> Result<Option<Trigger>, TriggerRepositoryError>;

    async fn list_enabled_cron(&self) -> Result<Vec<Trigger>, TriggerRepositoryError>;

    async fn create(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger: NewTrigger,
    ) -> Result<Trigger, TriggerRepositoryError>;

    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger: Trigger,
    ) -> Result<Trigger, TriggerRepositoryError>;

    async fn delete(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
    ) -> Result<bool, TriggerRepositoryError>;

    async fn mark_fired(
        &self,
        transaction: &UnitOfWorkTransaction,
        trigger_id: Uuid,
        fired_at: DateTime<FixedOffset>,
    ) -> Result<bool, TriggerRepositoryError>;
}

pub type SharedTriggerRepository = std::sync::Arc<dyn TriggerRepository + Send + Sync>;
