use thiserror::Error;

use crate::unit_of_work::UnitOfWorkError;

use super::graph::AgentGraphError;
const SLUG_PATTERN_DESC: &str = "^[a-z0-9][a-z0-9_-]*$";

#[derive(Debug, Error)]
pub enum AgentConfigRepositoryError {
    #[error(transparent)]
    Database(#[from] crate::persistence::PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[derive(Debug, Error)]
pub enum AgentConfigUseCaseError {
    #[error("purpose must match {SLUG_PATTERN_DESC} (got '{0}')")]
    InvalidPurpose(String),
    #[error("skill name must match {SLUG_PATTERN_DESC} (got '{0}')")]
    InvalidSkillName(String),
    #[error("agent_config with purpose '{0}' already exists")]
    DuplicatePurpose(String),
    #[error("agent_config '{0}' not found")]
    NotFound(String),
    #[error("skill '{0}' not found")]
    SkillNotFound(String),
    #[error(transparent)]
    InvalidAgentGraph(#[from] AgentGraphError),
    #[error(transparent)]
    Repository(#[from] AgentConfigRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
