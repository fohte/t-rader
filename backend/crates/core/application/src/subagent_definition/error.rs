use thiserror::Error;

use crate::agent_config::{AgentConfigRepositoryError, graph::AgentGraphError};
use crate::unit_of_work::UnitOfWorkError;

const KEY_PATTERN_DESC: &str = "^[a-z0-9][a-z0-9_-]*$";

#[derive(Debug, Error)]
pub enum SubagentDefinitionRepositoryError {
    #[error(transparent)]
    Database(#[from] crate::persistence::PersistenceError),
    #[error("stored subagent definition field {field} is invalid: {source}")]
    InvalidStoredValue {
        field: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[derive(Debug, Error)]
pub enum SubagentDefinitionUseCaseError {
    #[error("subagent definition key must match {KEY_PATTERN_DESC} (got '{0}')")]
    InvalidKey(String),
    #[error("subagent definition with key '{0}' already exists")]
    DuplicateKey(String),
    #[error("subagent definition '{0}' not found")]
    NotFound(String),
    #[error("subagent definition model must not be empty")]
    EmptyModel,
    #[error("subagent definition prompt must not be empty")]
    EmptyPrompt,
    #[error(
        "subagent definition '{key}' is referenced by agent_config '{purpose}' phase '{phase}'"
    )]
    Referenced {
        key: String,
        purpose: String,
        phase: String,
    },
    #[error("subagent definition tools must not include 'run_subagent'")]
    RunSubagentToolNotAllowed,
    #[error("subagent definition timeout_minutes must be between 1 and {max}, got {value}", max = crate::agent_config::graph::MAX_TIMEOUT_MINUTES)]
    TimeoutMinutesOutOfRange { value: i32 },
    #[error("subagent definition max_model_calls must be greater than 0, got {0}")]
    MaxModelCallsOutOfRange(i64),
    #[error("subagent definition tool_limits must be non-negative (got {tool} = {value})")]
    ToolLimitOutOfRange { tool: String, value: i64 },
    #[error("subagent definition tool_limits references undeclared tool '{0}'")]
    UndeclaredToolLimit(String),
    #[error(transparent)]
    InvalidAgentGraph(#[from] AgentGraphError),
    #[error(transparent)]
    AgentConfigRepository(#[from] AgentConfigRepositoryError),
    #[error(transparent)]
    Repository(#[from] SubagentDefinitionRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
}
