use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use core_application::saved_screen::{SavedScreenRepositoryError, SavedScreenUseCaseError};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{SaveScreenRequest, SavedScreenResponse};

fn map_error(error: SavedScreenUseCaseError) -> AppError {
    match error {
        SavedScreenUseCaseError::Validation(message) => AppError::Validation(message),
        SavedScreenUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("saved screen {id} not found"))
        }
        SavedScreenUseCaseError::Conflict(message) => AppError::Conflict(message),
        SavedScreenUseCaseError::Repository(SavedScreenRepositoryError::Persistence(error)) => {
            error.into()
        }
        SavedScreenUseCaseError::Repository(error) => AppError::Internal(error.to_string()),
    }
}

/// 保存したスクリーナー条件の一覧
#[utoipa::path(
    get,
    path = "/api/saved-screens",
    tag = "saved_screens",
    responses(
        (status = 200, body = Vec<SavedScreenResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_saved_screens(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<SavedScreenResponse>>, AppError> {
    let screens = state
        .saved_screen_use_cases
        .list()
        .await
        .map_err(map_error)?;
    Ok(Json(screens.into_iter().map(Into::into).collect()))
}

/// スクリーナー条件を保存
#[utoipa::path(
    post,
    path = "/api/saved-screens",
    tag = "saved_screens",
    request_body = SaveScreenRequest,
    responses(
        (status = 201, body = SavedScreenResponse),
        (status = 400, description = "保存条件が不正", body = ErrorResponse),
        (status = 409, description = "同名の保存条件が存在", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_saved_screen(
    State(state): State<FrontendApiState>,
    JsonBody(request): JsonBody<SaveScreenRequest>,
) -> Result<(StatusCode, Json<SavedScreenResponse>), AppError> {
    let screen = state
        .saved_screen_use_cases
        .create(request.into())
        .await
        .map_err(map_error)?;
    Ok((StatusCode::CREATED, Json(screen.into())))
}

/// 保存したスクリーナー条件を置き換える
#[utoipa::path(
    put,
    path = "/api/saved-screens/{id}",
    tag = "saved_screens",
    params(("id" = Uuid, Path, description = "保存条件 ID")),
    request_body = SaveScreenRequest,
    responses(
        (status = 200, body = SavedScreenResponse),
        (status = 400, description = "保存条件が不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "同名の保存条件が存在", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_saved_screen(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(request): JsonBody<SaveScreenRequest>,
) -> Result<Json<SavedScreenResponse>, AppError> {
    let screen = state
        .saved_screen_use_cases
        .update(id, request.into())
        .await
        .map_err(map_error)?;
    Ok(Json(screen.into()))
}

/// 保存したスクリーナー条件を削除
#[utoipa::path(
    delete,
    path = "/api/saved-screens/{id}",
    tag = "saved_screens",
    params(("id" = Uuid, Path, description = "保存条件 ID")),
    responses(
        (status = 204),
        (status = 400, description = "パスパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_saved_screen(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .saved_screen_use_cases
        .delete(id)
        .await
        .map_err(map_error)?;
    Ok(StatusCode::NO_CONTENT)
}
