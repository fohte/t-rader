//! 分類軸 CRUD の REST handler。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::group_axis::{CreateGroupAxisCommand, UpdateGroupAxisCommand};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::GroupAxisResponse;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateGroupAxisRequest {
    pub key: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub sync_source: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateGroupAxisRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    pub sync_source: Option<Option<String>>,
}

/// 分類軸一覧
#[utoipa::path(
    get,
    path = "/api/group-axes",
    tag = "group_axes",
    responses(
        (status = 200, body = Vec<GroupAxisResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_group_axes(
    State(state): State<AppState>,
) -> Result<Json<Vec<GroupAxisResponse>>, AppError> {
    Ok(Json(
        state
            .group_axis_use_cases
            .list()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// 分類軸を作成
#[utoipa::path(
    post,
    path = "/api/group-axes",
    tag = "group_axes",
    request_body = CreateGroupAxisRequest,
    responses(
        (status = 201, body = GroupAxisResponse),
        (status = 400, body = ErrorResponse),
        (status = 409, description = "key が既存と衝突", body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_group_axis(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateGroupAxisRequest>,
) -> Result<(StatusCode, Json<GroupAxisResponse>), AppError> {
    let created = state
        .group_axis_use_cases
        .create(CreateGroupAxisCommand {
            key: payload.key,
            name: payload.name,
            description: payload.description,
            sync_source: payload.sync_source,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// 分類軸を取得
#[utoipa::path(
    get,
    path = "/api/group-axes/{key}",
    tag = "group_axes",
    params(("key" = String, Path, description = "分類軸 key")),
    responses(
        (status = 200, body = GroupAxisResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_group_axis(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
) -> Result<Json<GroupAxisResponse>, AppError> {
    let axis = state.group_axis_use_cases.get(&key).await?;
    Ok(Json(axis.into()))
}

/// 分類軸を部分更新する (key は変更不可)
#[utoipa::path(
    patch,
    path = "/api/group-axes/{key}",
    tag = "group_axes",
    params(("key" = String, Path, description = "分類軸 key")),
    request_body = UpdateGroupAxisRequest,
    responses(
        (status = 200, body = GroupAxisResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "リクエストの Content-Type が application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_group_axis(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateGroupAxisRequest>,
) -> Result<Json<GroupAxisResponse>, AppError> {
    let axis = state
        .group_axis_use_cases
        .update(
            &key,
            UpdateGroupAxisCommand {
                name: payload.name,
                description: payload.description,
                sync_source: payload.sync_source,
            },
        )
        .await?;
    Ok(Json(axis.into()))
}

/// グループが残っている分類軸は削除しない
#[utoipa::path(
    delete,
    path = "/api/group-axes/{key}",
    tag = "group_axes",
    params(("key" = String, Path, description = "分類軸 key")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "分類軸にグループが残っている", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_group_axis(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state.group_axis_use_cases.delete(&key).await?;
    Ok(StatusCode::NO_CONTENT)
}
