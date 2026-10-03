use axum::extract::FromRequestParts;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
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

/// Axum の `Json` extractor のラッパー。
/// リクエストボディのデシリアライズに失敗した場合に、`text/plain` ではなく
/// `application/json` でエラーレスポンスを返す。
pub struct JsonBody<T>(pub T);

/// `JsonBody` のパース失敗時に返す JSON エラー
pub struct JsonBodyRejection(JsonRejection);

impl IntoResponse for JsonBodyRejection {
    fn into_response(self) -> Response {
        let status = self.0.status();
        let body = ErrorResponse {
            error: self.0.body_text(),
        };

        (status, axum::Json(body)).into_response()
    }
}

impl<S, T> axum::extract::FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = JsonBodyRejection;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(req, state)
            .await
            .map(|axum::Json(v)| JsonBody(v))
            .map_err(JsonBodyRejection)
    }
}

/// Axum の `Query` extractor のラッパー。
/// クエリパラメータのデシリアライズに失敗した場合に、`text/plain` ではなく
/// `application/json` でエラーレスポンスを返す。
pub struct JsonQuery<T>(pub T);

/// `JsonQuery` のパース失敗時に返す JSON エラー
pub struct JsonQueryRejection(QueryRejection);

impl IntoResponse for JsonQueryRejection {
    fn into_response(self) -> Response {
        let status = self.0.status();
        let body = ErrorResponse {
            error: self.0.body_text(),
        };

        (status, axum::Json(body)).into_response()
    }
}

impl<S, T> FromRequestParts<S> for JsonQuery<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = JsonQueryRejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(v)| JsonQuery(v))
            .map_err(JsonQueryRejection)
    }
}
