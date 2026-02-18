use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::data_provider::DataProviderError;

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

    #[error("data provider error: {0}")]
    DataProvider(#[from] DataProviderError),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
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
            AppError::Database(_) | AppError::Config(_) => {
                // 内部エラーの詳細はログに記録し、クライアントには汎用メッセージのみ返す
                tracing::error!("{self}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
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
