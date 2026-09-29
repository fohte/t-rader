use thiserror::Error;
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkError;

use super::repository::StrategyTaskRepositoryError;

#[derive(Debug, Error)]
pub enum SubmitTaskError {
    #[error("strategy {0} not found")]
    StrategyNotFound(Uuid),
    #[error("prompt must not be empty")]
    EmptyPrompt,
    #[error("agent_config for purpose '{0}' not found")]
    PurposeNotFound(String),
    #[error(transparent)]
    Repository(#[from] StrategyTaskRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    AgentTask(#[from] crate::agent_task_client::AgentTaskError),
}

#[derive(Debug, Error)]
pub enum ResumeTaskError {
    #[error("strategy task {0} not found")]
    NotFound(Uuid),
    #[error("strategy task {0} is not resumable (current phase: {1})")]
    NotResumable(Uuid, &'static str),
    #[error(transparent)]
    Repository(#[from] StrategyTaskRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    AgentTask(#[from] crate::agent_task_client::AgentTaskError),
}

#[derive(Debug, Error)]
pub enum GetTaskError {
    #[error("strategy task {0} not found")]
    NotFound(Uuid),
    #[error("strategy task {task_id} does not belong to strategy {strategy_id}")]
    StrategyMismatch { task_id: Uuid, strategy_id: Uuid },
    #[error(transparent)]
    Repository(#[from] StrategyTaskRepositoryError),
}

#[derive(Debug, Error)]
pub enum ListTasksError {
    #[error(transparent)]
    Repository(#[from] StrategyTaskRepositoryError),
}

#[derive(Debug, Error)]
pub enum ReconcileTaskError {
    #[error(transparent)]
    Repository(#[from] StrategyTaskRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    AgentTask(#[from] crate::agent_task_client::AgentTaskError),
}
