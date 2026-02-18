use axum::extract::FromRequestParts;
use axum::extract::rejection::PathRejection;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;

use crate::error::ErrorResponse;

/// Axum の `Path` extractor のラッパー。
/// パスパラメータのパースに失敗した場合に、`text/plain` ではなく
/// `application/json` でエラーレスポンスを返す。
pub struct JsonPath<T>(pub T);

/// `JsonPath` のパース失敗時に返す JSON エラー
pub struct JsonPathRejection(PathRejection);

impl IntoResponse for JsonPathRejection {
    fn into_response(self) -> Response {
        let status = self.0.status();
        let body = ErrorResponse {
            error: self.0.body_text(),
        };

        (status, axum::Json(body)).into_response()
    }
}

impl<S, T> FromRequestParts<S> for JsonPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = JsonPathRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(v)| JsonPath(v))
            .map_err(JsonPathRejection)
    }
}
