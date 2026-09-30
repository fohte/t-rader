use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use core_application::change_history::ChangeHistoryError;
use core_application::note::{NoteRepositoryError, NoteUseCaseError};
use core_application::note_kind::{NoteKindRepositoryError, NoteKindUseCaseError};
use core_application::persistence::PersistenceError;
use core_application::strategy_existence::StrategyExistenceError;
use core_application::unit_of_work::UnitOfWorkError;
use sea_orm::{DbErr, RuntimeErr, SqlErr};
use serde::Serialize;
use utoipa::ToSchema;

use crate::data_provider::{
    DailyBarSourceError, EquityMasterSourceError, MarketDailyBarSourceError,
};

// SeaORM の `SqlErr` で拾えない PostgreSQL SQLSTATE を補完する。
// NOT NULL 違反 (23502) は handler 側の入力検証漏れまたは型不整合を示すサーバーバグなので
// あえて含めず、デフォルトの 500 にフォールバックさせて顕在化させる。
// ref: https://www.postgresql.org/docs/current/errcodes-appendix.html
const PG_UNIQUE_VIOLATION: &str = "23505";
const PG_CHECK_VIOLATION: &str = "23514";

/// DB 制約違反系エラーを HTTP ステータスにマップする。
/// 該当しない場合は `None` を返し、呼び出し側で 500 にフォールバックさせる。
fn classify_db_constraint(err: &DbErr) -> Option<(StatusCode, String)> {
    if let Some(sql_err) = err.sql_err() {
        match sql_err {
            SqlErr::ForeignKeyConstraintViolation(_) => {
                return Some((
                    StatusCode::BAD_REQUEST,
                    "referenced resource does not exist".to_string(),
                ));
            }
            SqlErr::UniqueConstraintViolation(_) => {
                return Some((StatusCode::CONFLICT, "resource already exists".to_string()));
            }
            _ => {}
        }
    }

    // SeaORM の `sql_err()` は partial unique index 由来の違反などを取りこぼすことがあるため、
    // raw SQLSTATE でも判定する。check 違反 (例: qty > 0) は handler 側のビジネスルール表現として
    // 400 にマップする。
    let (DbErr::Exec(RuntimeErr::SqlxError(sqlx_err))
    | DbErr::Query(RuntimeErr::SqlxError(sqlx_err))) = err
    else {
        return None;
    };
    let code = sqlx_err.as_database_error()?.code()?;
    match code.as_ref() {
        PG_UNIQUE_VIOLATION => Some((StatusCode::CONFLICT, "resource already exists".to_string())),
        PG_CHECK_VIOLATION => Some((
            StatusCode::BAD_REQUEST,
            "value violates database constraint".to_string(),
        )),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),

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

impl From<PersistenceError> for AppError {
    fn from(error: PersistenceError) -> Self {
        match error {
            PersistenceError::Database(message) => Self::Database(sea_orm::DbErr::Custom(message)),
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
            other => Self::Database(DbErr::Custom(other.to_string())),
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
        other => AppError::Database(DbErr::Custom(other.to_string())),
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
            AppError::Database(db_err) => {
                if matches!(db_err, DbErr::RecordNotUpdated) {
                    // read-then-update の間に対象行が並行削除されると 0 行更新でここに来る。
                    // クライアントからは「対象が既に存在しない」だけなので 404 として扱う。
                    (StatusCode::NOT_FOUND, "resource not found".to_string())
                } else if let Some(mapped) = classify_db_constraint(db_err) {
                    mapped
                } else {
                    // 内部エラーの詳細はログに記録し、クライアントには汎用メッセージのみ返す
                    tracing::error!("{self}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal server error".to_string(),
                    )
                }
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

    #[rstest]
    #[case::record_not_updated(
        DbErr::RecordNotUpdated,
        StatusCode::NOT_FOUND,
        json!({ "error": "resource not found" })
    )]
    #[case::other_db_error(
        DbErr::Custom("unexpected".into()),
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": "internal server error" })
    )]
    #[tokio::test]
    async fn test_database_error_response(
        #[case] db_err: DbErr,
        #[case] expected_status: StatusCode,
        #[case] expected_body: serde_json::Value,
    ) {
        let response = AppError::Database(db_err).into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("parse json body");
        assert_eq!((status, body), (expected_status, expected_body));
    }
}
