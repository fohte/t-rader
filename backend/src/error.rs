use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use sea_orm::{DbErr, RuntimeErr, SqlErr};
use serde::Serialize;
use utoipa::ToSchema;

use crate::data_provider::DataProviderError;

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

    #[error("data provider error: {0}")]
    DataProvider(#[from] DataProviderError),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),
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
            AppError::DataProvider(e) => match e {
                DataProviderError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
                DataProviderError::RateLimited { .. } => {
                    tracing::error!("{self}");
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        "service temporarily unavailable".to_string(),
                    )
                }
                _ => {
                    tracing::error!("{self}");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal server error".to_string(),
                    )
                }
            },
        };

        let body = ErrorResponse { error: message };

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_service_unavailable_returns_503() {
        let error = AppError::ServiceUnavailable("data provider is not configured".into());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[rstest]
    #[case::record_not_updated(DbErr::RecordNotUpdated, StatusCode::NOT_FOUND)]
    #[case::other_db_error(
        DbErr::Custom("unexpected".into()),
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    fn test_database_error_status(#[case] db_err: DbErr, #[case] expected: StatusCode) {
        let response = AppError::Database(db_err).into_response();
        assert_eq!(response.status(), expected);
    }
}
