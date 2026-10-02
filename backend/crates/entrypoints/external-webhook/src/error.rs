use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use core_application::agent_task_client::AgentTaskError;
use core_application::persistence::PersistenceError;
use core_application::strategy::StrategyRepositoryError;
use core_application::strategy_existence::StrategyExistenceError;
use core_application::strategy_task::{StrategyTaskRepositoryError, SubmitTaskError};
use core_application::trigger::{TriggerRepositoryError, TriggerUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("internal error: {0}")]
    Internal(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
}

/// API エラーレスポンスの JSON 構造。
#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    /// エラーメッセージ
    pub error: String,
}

impl From<PersistenceError> for AppError {
    fn from(error: PersistenceError) -> Self {
        match error {
            PersistenceError::Database(message) => Self::Internal(message),
            PersistenceError::MissingReference(_) => {
                Self::Validation("referenced resource does not exist".into())
            }
            PersistenceError::Conflict(_) => Self::Conflict("resource already exists".into()),
            PersistenceError::ConstraintViolation(_) => {
                Self::Validation("value violates database constraint".into())
            }
            PersistenceError::RecordNotUpdated(_) => Self::NotFound("resource not found".into()),
        }
    }
}

pub(crate) fn map_hook_error(hook_slug: &str, error: TriggerUseCaseError) -> AppError {
    match error {
        TriggerUseCaseError::NotFound(_)
        | TriggerUseCaseError::HookNotFound(_)
        | TriggerUseCaseError::Disabled(_)
        | TriggerUseCaseError::NoStrategy(_) => {
            AppError::NotFound(format!("hook {hook_slug} not found"))
        }
        TriggerUseCaseError::Submit(error) => map_submit_error(error),
        TriggerUseCaseError::Repository(TriggerRepositoryError::Database(error))
        | TriggerUseCaseError::StrategyRepository(StrategyRepositoryError::Database(error))
        | TriggerUseCaseError::StrategyExistence(StrategyExistenceError::Database(error))
        | TriggerUseCaseError::UnitOfWork(
            UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}

pub(crate) fn map_submit_error(error: SubmitTaskError) -> AppError {
    match error {
        SubmitTaskError::EmptyPrompt => AppError::Validation("prompt must not be empty".into()),
        SubmitTaskError::StrategyNotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        SubmitTaskError::PurposeNotFound(purpose) => {
            AppError::ServiceUnavailable(format!("agent_config for purpose '{purpose}' not found"))
        }
        SubmitTaskError::Repository(StrategyTaskRepositoryError::Database(error))
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::Begin(error))
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        SubmitTaskError::Repository(StrategyTaskRepositoryError::InvalidTransaction)
        | SubmitTaskError::UnitOfWork(UnitOfWorkError::InvalidTransaction) => {
            AppError::Internal("invalid strategy task transaction".into())
        }
        SubmitTaskError::AgentTask(AgentTaskError::NotConfigured) => {
            AppError::ServiceUnavailable("agent task client is not configured".into())
        }
        SubmitTaskError::AgentTask(agent_error) => {
            AppError::Config(format!("agent task error: {agent_error}"))
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Internal(_) | AppError::Config(_) => {
                tracing::error!("{self}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::NotFound(message) => (StatusCode::NOT_FOUND, message.clone()),
            AppError::Validation(message) => (StatusCode::BAD_REQUEST, message.clone()),
            AppError::Conflict(message) => (StatusCode::CONFLICT, message.clone()),
            AppError::ServiceUnavailable(message) => {
                (StatusCode::SERVICE_UNAVAILABLE, message.clone())
            }
        };

        (status, axum::Json(ErrorResponse { error: message })).into_response()
    }
}
