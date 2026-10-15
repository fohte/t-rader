use axum::Json;
use axum::extract::State;
use core_application::persistence::PersistenceError;
use core_application::widget::{WidgetQueryError, WidgetUseCaseError};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{WidgetQueryRequest, WidgetQueryResponse};

fn map_error(error: WidgetUseCaseError) -> AppError {
    match error {
        WidgetUseCaseError::Validation(errors) => AppError::Validation(errors.to_string()),
        WidgetUseCaseError::NoData => AppError::NotFound(
            "no daily bar data is available on or before the requested date".to_string(),
        ),
        WidgetUseCaseError::Query(WidgetQueryError::Database(error)) => match error {
            database_error @ PersistenceError::Database(_) => {
                AppError::Internal(database_error.to_string())
            }
            error => error.into(),
        },
    }
}

/// ウィジェット定義を評価して描画データを返す。
#[utoipa::path(
    post,
    path = "/api/widgets/query",
    tag = "widgets",
    request_body = WidgetQueryRequest,
    responses(
        (status = 200, body = WidgetQueryResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn query_widget(
    State(state): State<FrontendApiState>,
    JsonBody(request): JsonBody<WidgetQueryRequest>,
) -> Result<Json<WidgetQueryResponse>, AppError> {
    state
        .widget_use_cases
        .query(request.into())
        .await
        .map(Into::into)
        .map(Json)
        .map_err(map_error)
}
