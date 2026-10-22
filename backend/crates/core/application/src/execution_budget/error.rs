use thiserror::Error;

use crate::{persistence::PersistenceError, unit_of_work::UnitOfWorkError};

#[derive(Debug, Error)]
pub enum ExecutionBudgetRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
    #[error("execution budget state was not found")]
    StateNotFound,
    #[error("execution budget deadline has expired")]
    DeadlineExpired,
    #[error("execution reservation identifier conflicts with an existing call")]
    ReservationConflict,
    #[error("execution budget limit was exceeded for {resource_key}")]
    LimitExceeded { resource_key: String },
    #[error("execution reservation cannot transition from its current state")]
    InvalidReservationState,
}

#[derive(Debug, Error)]
pub enum ExecutionBudgetUseCaseError {
    #[error(transparent)]
    Repository(#[from] ExecutionBudgetRepositoryError),
    #[error(transparent)]
    UnitOfWork(#[from] UnitOfWorkError),
    #[error(transparent)]
    StrategyExecution(#[from] crate::strategy_execution::StrategyExecutionRepositoryError),
    #[error("task start snapshot is missing")]
    MissingTaskStartSnapshot,
    #[error("task start snapshot is invalid")]
    InvalidTaskStartSnapshot,
    #[error("execution budget request is invalid")]
    InvalidRequest,
}
