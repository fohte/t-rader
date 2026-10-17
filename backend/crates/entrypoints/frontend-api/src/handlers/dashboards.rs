use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::dashboard::{DashboardRepositoryError, DashboardUseCaseError};

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateDashboardRequest, DashboardResponse, UpdateDashboardRequest};

fn map_error(error: DashboardUseCaseError) -> AppError {
    match error {
        DashboardUseCaseError::Validation(message) => AppError::Validation(message),
        DashboardUseCaseError::NotFound(key) => {
            AppError::NotFound(format!("dashboard '{key}' not found"))
        }
        DashboardUseCaseError::Conflict(message) => AppError::Conflict(message),
        DashboardUseCaseError::Repository(DashboardRepositoryError::Persistence(error)) => {
            error.into()
        }
        DashboardUseCaseError::Repository(error) => AppError::Internal(error.to_string()),
    }
}

/// ダッシュボード一覧を取得する。
#[utoipa::path(
    get,
    path = "/api/dashboards",
    tag = "dashboards",
    responses(
        (status = 200, body = Vec<DashboardResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_dashboards(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<DashboardResponse>>, AppError> {
    let dashboards = state.dashboard_use_cases.list().await.map_err(map_error)?;
    Ok(Json(dashboards.into_iter().map(Into::into).collect()))
}

/// ダッシュボードを作成する。
#[utoipa::path(
    post,
    path = "/api/dashboards",
    tag = "dashboards",
    request_body = CreateDashboardRequest,
    responses(
        (status = 201, body = DashboardResponse),
        (status = 400, description = "ダッシュボード設定が不正", body = ErrorResponse),
        (status = 409, description = "同じ key のダッシュボードが存在", body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_dashboard(
    State(state): State<FrontendApiState>,
    JsonBody(request): JsonBody<CreateDashboardRequest>,
) -> Result<(StatusCode, Json<DashboardResponse>), AppError> {
    let dashboard = state
        .dashboard_use_cases
        .create(request.into())
        .await
        .map_err(map_error)?;
    Ok((StatusCode::CREATED, Json(dashboard.into())))
}

/// 指定 key のダッシュボードを取得する。
#[utoipa::path(
    get,
    path = "/api/dashboards/{key}",
    tag = "dashboards",
    params(("key" = String, Path, description = "ダッシュボード key")),
    responses(
        (status = 200, body = DashboardResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_dashboard(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
) -> Result<Json<DashboardResponse>, AppError> {
    let dashboard = state
        .dashboard_use_cases
        .get(&key)
        .await
        .map_err(map_error)?;
    Ok(Json(dashboard.into()))
}

/// ダッシュボード設定を置き換える。
#[utoipa::path(
    put,
    path = "/api/dashboards/{key}",
    tag = "dashboards",
    params(("key" = String, Path, description = "ダッシュボード key")),
    request_body = UpdateDashboardRequest,
    responses(
        (status = 200, body = DashboardResponse),
        (status = 400, description = "ダッシュボード設定が不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_dashboard(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(request): JsonBody<UpdateDashboardRequest>,
) -> Result<Json<DashboardResponse>, AppError> {
    let dashboard = state
        .dashboard_use_cases
        .update(&key, request.config)
        .await
        .map_err(map_error)?;
    Ok(Json(dashboard.into()))
}

/// ダッシュボードを削除する。
#[utoipa::path(
    delete,
    path = "/api/dashboards/{key}",
    tag = "dashboards",
    params(("key" = String, Path, description = "ダッシュボード key")),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_dashboard(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state
        .dashboard_use_cases
        .delete(&key)
        .await
        .map_err(map_error)?;
    Ok(StatusCode::NO_CONTENT)
}
