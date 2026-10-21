use thiserror::Error;

use crate::agent_config::AgentConfigUseCaseError;
use crate::custom_indicator::CustomIndicatorUseCaseError;
use crate::custom_indicator_evaluation::CustomIndicatorEvaluationUseCaseError;
use crate::persistence::PersistenceError;
use crate::strategy_task::StrategyTaskRepositoryError;
use crate::subagent_definition::SubagentDefinitionUseCaseError;
use crate::unit_of_work::UnitOfWorkError;

#[derive(Debug, Error)]
pub enum StrategyExecutionRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("payload state does not allow this operation")]
    InvalidPayloadState,
    #[error("payload size limit exceeded")]
    PayloadTooLarge,
    #[error("payload chunks are inconsistent")]
    InconsistentPayload,
}

#[derive(Debug, Error)]
pub enum DatasetProviderError {
    #[error("dataset provider failed: {0}")]
    Failed(String),
}

#[derive(Debug, Error)]
pub enum StrategyExecutionUseCaseError {
    #[error(transparent)]
    Repository(#[from] StrategyExecutionRepositoryError),
    #[error(transparent)]
    TaskRepository(#[from] StrategyTaskRepositoryError),
    #[error(transparent)]
    SubagentDefinitions(#[from] SubagentDefinitionUseCaseError),
    #[error(transparent)]
    CustomIndicator(#[from] CustomIndicatorUseCaseError),
    #[error(transparent)]
    CustomIndicatorEvaluation(#[from] CustomIndicatorEvaluationUseCaseError),
    #[error(transparent)]
    AgentConfig(#[from] AgentConfigUseCaseError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error("task execution was not found")]
    TaskNotFound,
    #[error("task execution is outside the requested strategy scope")]
    StrategyMismatch,
    #[error("task execution is not running")]
    TaskNotRunning,
    #[error("definition snapshot is invalid")]
    InvalidDefinitionSnapshot,
    #[error("custom indicator definition was not found")]
    CustomIndicatorNotFound,
    #[error("payload was not found in this task")]
    PayloadNotFound,
    #[error("payload is not sealed")]
    PayloadNotSealed,
    #[error("payload schema is invalid: {0}")]
    InvalidPayloadSchema(String),
    #[error("payload does not match its schema: {0}")]
    PayloadSchemaMismatch(String),
    #[error("payload pointer does not resolve")]
    InvalidPayloadPointer,
    #[error("payload page limit is invalid")]
    InvalidPageLimit,
    #[error("payload page exceeds the response byte limit")]
    PayloadPageTooLarge,
    #[error("payload source revision changed between pages")]
    SourceRevisionChanged,
    #[error("dataset provider returned an invalid cursor")]
    InvalidDatasetCursor,
    #[error("dataset provider returned inconsistent completion counts")]
    InvalidDatasetCompletion,
    #[error("dataset provider reported an incomplete capture")]
    DatasetCaptureIncomplete,
    #[error(transparent)]
    DatasetProvider(#[from] DatasetProviderError),
    #[error("numeric projection is invalid")]
    InvalidNumericProjection,
    #[error("payload could not be serialized")]
    Serialization,
}
