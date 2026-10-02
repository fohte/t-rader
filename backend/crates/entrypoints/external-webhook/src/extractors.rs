use axum::extract::FromRequest;
use axum::extract::FromRequestParts;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;

use crate::error::ErrorResponse;

pub struct JsonBody<T>(pub T);

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

impl<S, T> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = JsonBodyRejection;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        axum::Json::<T>::from_request(request, state)
            .await
            .map(|axum::Json(value)| Self(value))
            .map_err(JsonBodyRejection)
    }
}

pub struct JsonPath<T>(pub T);

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
            .map(|axum::extract::Path(value)| Self(value))
            .map_err(JsonPathRejection)
    }
}
