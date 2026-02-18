use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ErrorResponse;

/// リクエストボディに null バイト (\0) が含まれている場合に 400 を返すミドルウェア。
/// PostgreSQL の TEXT/VARCHAR カラムは null バイトを受け付けないため、
/// ハンドラに到達する前に一括で拒否する。
pub async fn reject_null_bytes(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();

    let bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(ErrorResponse {
                    error: "request body too large".to_string(),
                }),
            )
                .into_response();
        }
    };

    if bytes.contains(&0) {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "request body must not contain null bytes".to_string(),
            }),
        )
            .into_response();
    }

    let request = Request::from_parts(parts, Body::from(bytes));
    next.run(request).await
}
