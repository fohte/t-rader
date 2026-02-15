use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("configuration error: {0}")]
    Config(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Database(_) | AppError::Migration(_) | AppError::Config(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        // 内部エラーの詳細はログに記録し、クライアントには汎用メッセージのみ返す
        tracing::error!("{self}");

        let body = serde_json::json!({
            "error": "internal server error",
        });

        (status, axum::Json(body)).into_response()
    }
}
