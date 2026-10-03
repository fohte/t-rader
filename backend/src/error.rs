use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use core_application::account_risk_policy::AccountRiskPolicyDataError;
use core_application::bars::{BarsRepositoryError, BarsUseCaseError};
use core_application::change_history::ChangeHistoryError;
use core_application::group_axis::{GroupAxisRepositoryError, GroupAxisUseCaseError};
use core_application::note::{NoteRepositoryError, NoteUseCaseError};
use core_application::note_kind::{NoteKindRepositoryError, NoteKindUseCaseError};
use core_application::persistence::PersistenceError;
use core_application::strategy_existence::StrategyExistenceError;
use core_application::unit_of_work::UnitOfWorkError;
use serde::Serialize;
use utoipa::ToSchema;

use crate::data_provider::{
    DailyBarSourceError, EquityMasterSourceError, MarketDailyBarSourceError,
};

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

    #[error("daily bar source error: {0}")]
    DailyBarSource(#[from] DailyBarSourceError),

    #[error("{0}")]
    EquityMasterSource(#[from] EquityMasterSourceError),

    #[error("{0}")]
    MarketDailyBarSource(#[from] MarketDailyBarSourceError),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),
}

impl From<AccountRiskPolicyDataError> for AppError {
    fn from(error: AccountRiskPolicyDataError) -> Self {
        match error {
            AccountRiskPolicyDataError::Validation(message) => Self::Validation(message),
            error @ AccountRiskPolicyDataError::InvalidData(_) => Self::Internal(error.to_string()),
        }
    }
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

impl From<BarsUseCaseError> for AppError {
    fn from(error: BarsUseCaseError) -> Self {
        match error {
            BarsUseCaseError::Repository(BarsRepositoryError::Database(error)) => error.into(),
            BarsUseCaseError::UnitOfWork(
                UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error),
            ) => error.into(),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<NoteKindUseCaseError> for AppError {
    fn from(error: NoteKindUseCaseError) -> Self {
        match error {
            NoteKindUseCaseError::Validation(message) => Self::Validation(message),
            NoteKindUseCaseError::NotFound(message) => Self::NotFound(message),
            NoteKindUseCaseError::Conflict(message) => Self::Conflict(message),
            NoteKindUseCaseError::Repository(NoteKindRepositoryError::Database(error))
            | NoteKindUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
            | NoteKindUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
            | NoteKindUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
            NoteKindUseCaseError::Note(error) => map_note_use_case_error(error),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<GroupAxisUseCaseError> for AppError {
    fn from(error: GroupAxisUseCaseError) -> Self {
        match error {
            GroupAxisUseCaseError::Validation(message) => Self::Validation(message),
            GroupAxisUseCaseError::NotFound(message) => Self::NotFound(message),
            GroupAxisUseCaseError::Conflict(message) => Self::Conflict(message),
            GroupAxisUseCaseError::Repository(GroupAxisRepositoryError::Database(error))
            | GroupAxisUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
            | GroupAxisUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
            other => Self::Internal(other.to_string()),
        }
    }
}

fn map_note_use_case_error(error: NoteUseCaseError) -> AppError {
    match error {
        NoteUseCaseError::Validation(message) => AppError::Validation(message),
        NoteUseCaseError::UnknownNoteKind(kind) => {
            AppError::Validation(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::NotFound(message) => AppError::NotFound(message),
        NoteUseCaseError::ReferencedNoteKindNotFound(key) => {
            AppError::NotFound(format!("note kind {key} not found"))
        }
        NoteUseCaseError::Conflict(message) => AppError::Conflict(message),
        NoteUseCaseError::Repository(NoteRepositoryError::Database(error))
        | NoteUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error))
        | NoteUseCaseError::StrategyExistence(StrategyExistenceError::Database(error)) => {
            error.into()
        }
        other => AppError::Internal(other.to_string()),
    }
}

/// API エラーレスポンスの JSON 構造
#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    /// エラーメッセージ
    pub error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Internal(_) => {
                tracing::error!("{self}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::Config(_) => {
                tracing::error!("{self}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::DailyBarSource(DailyBarSourceError::NotFound(msg)) => {
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::DailyBarSource(DailyBarSourceError::RateLimited(_)) => {
                tracing::error!("{self}");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service temporarily unavailable".to_string(),
                )
            }
            AppError::DailyBarSource(DailyBarSourceError::Failed(_))
            | AppError::EquityMasterSource(_)
            | AppError::MarketDailyBarSource(_) => {
                tracing::error!("{self}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };

        let body = ErrorResponse { error: message };

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use core_application::account_risk_policy::{AccountRiskPolicyData, parse_risk_policy};
    use rstest::rstest;
    use serde_json::json;

    use super::*;

    #[rstest]
    #[case::not_found(
        DailyBarSourceError::NotFound("sample instrument not found".to_string()),
        StatusCode::NOT_FOUND,
        "sample instrument not found",
    )]
    #[case::rate_limited(
        DailyBarSourceError::RateLimited("source rate limit reached".to_string()),
        StatusCode::SERVICE_UNAVAILABLE,
        "service temporarily unavailable",
    )]
    #[case::failed(
        DailyBarSourceError::Failed("source request failed".to_string()),
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error",
    )]
    #[tokio::test]
    async fn daily_bar_source_error_maps_to_http_response(
        #[case] error: DailyBarSourceError,
        #[case] expected_status: StatusCode,
        #[case] expected_message: &str,
    ) {
        let response = AppError::from(error).into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let body = serde_json::from_slice::<serde_json::Value>(&body).expect("parse response body");

        assert_eq!(
            (status, body),
            (
                expected_status,
                serde_json::json!({ "error": expected_message }),
            ),
        );
    }

    #[rstest]
    fn test_service_unavailable_returns_503() {
        let error = AppError::ServiceUnavailable("data provider is not configured".into());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn internal_error_maps_to_http_internal_response() {
        let response = AppError::Internal("database unavailable".into()).into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json body");

        assert_eq!(
            (status, body),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal server error" }),
            ),
        );
    }

    #[tokio::test]
    async fn malformed_account_risk_policy_maps_to_http_internal_response() {
        let error =
            parse_risk_policy::<AccountRiskPolicyData>(json!({ "max_group_ratios": "invalid" }))
                .expect_err("malformed policy must fail to parse");
        let response = AppError::from(error).into_response();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body = serde_json::from_slice::<serde_json::Value>(&body).expect("parse json body");

        assert_eq!(
            (status, body),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "internal server error" }),
            ),
        );
    }
}
