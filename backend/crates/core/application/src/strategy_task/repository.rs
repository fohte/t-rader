use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{StrategyTask, StrategyTaskStep, StrategyTaskUpdate, TaskListQuery};

#[derive(Debug, Error)]
pub enum StrategyTaskRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait StrategyTaskRepository: Send + Sync {
    async fn strategy_exists(&self, strategy_id: Uuid)
    -> Result<bool, StrategyTaskRepositoryError>;
    async fn agent_config_exists(&self, purpose: &str)
    -> Result<bool, StrategyTaskRepositoryError>;
    async fn insert(
        &self,
        transaction: &UnitOfWorkTransaction,
        task: StrategyTask,
    ) -> Result<(), StrategyTaskRepositoryError>;
    async fn update(
        &self,
        transaction: &UnitOfWorkTransaction,
        task: StrategyTaskUpdate,
    ) -> Result<bool, StrategyTaskRepositoryError>;
    async fn apply_status_and_steps(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        task_update: Option<StrategyTaskUpdate>,
        steps: Option<serde_json::Value>,
    ) -> Result<bool, StrategyTaskRepositoryError>;
    async fn find_by_id(
        &self,
        task_id: Uuid,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError>;
    async fn find_by_a2a_task_id(
        &self,
        a2a_task_id: &str,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError>;
    async fn list(
        &self,
        query: TaskListQuery,
    ) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError>;
    async fn list_in_flight(&self) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError>;
    async fn list_steps(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError>;
    async fn claim_resumable(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        now: chrono::DateTime<chrono::FixedOffset>,
        mark_auto_resumed: bool,
    ) -> Result<bool, StrategyTaskRepositoryError>;
}

pub type SharedStrategyTaskRepository = Arc<dyn StrategyTaskRepository + Send + Sync>;
