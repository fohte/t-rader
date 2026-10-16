use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized: {0}")]
    Unauthorized(String),
}

/// API エラーレスポンスの JSON 構造。
#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    /// エラーメッセージ
    pub error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let AppError::Unauthorized(message) = self;

        (
            StatusCode::UNAUTHORIZED,
            axum::Json(ErrorResponse { error: message }),
        )
            .into_response()
    }
}
