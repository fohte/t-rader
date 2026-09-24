//! ノート種別 CRUD の REST handler。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Deserializer};
use utoipa::ToSchema;

use crate::AppState;
use crate::entities::note_kind;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::services::change_history::Actor;
use crate::services::note_kinds as svc;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateNoteKindRequest {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNoteKindRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_nullable_option")]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

fn deserialize_nullable_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// ノート種別一覧
#[utoipa::path(
    get,
    path = "/api/note-kinds",
    tag = "note_kinds",
    responses(
        (status = 200, body = Vec<note_kind::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_kinds(
    State(state): State<AppState>,
) -> Result<Json<Vec<note_kind::Model>>, AppError> {
    Ok(Json(svc::list(&state.db).await?))
}

/// ノート種別を作成
#[utoipa::path(
    post,
    path = "/api/note-kinds",
    tag = "note_kinds",
    request_body = CreateNoteKindRequest,
    responses(
        (status = 201, body = note_kind::Model),
        (status = 400, body = ErrorResponse),
        (status = 409, description = "key が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note_kind(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateNoteKindRequest>,
) -> Result<(StatusCode, Json<note_kind::Model>), AppError> {
    let created = svc::create(
        &state.db,
        Actor::Human,
        svc::CreateNoteKind {
            key: payload.key,
            display_name: payload.display_name,
            requires_approval: payload.requires_approval,
            description: payload.description,
            sort_order: payload.sort_order,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// ノート種別を部分更新する (key は変更不可)
#[utoipa::path(
    patch,
    path = "/api/note-kinds/{key}",
    tag = "note_kinds",
    params(("key" = String, Path, description = "ノート種別 key")),
    request_body = UpdateNoteKindRequest,
    responses(
        (status = 200, body = note_kind::Model),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note_kind(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateNoteKindRequest>,
) -> Result<Json<note_kind::Model>, AppError> {
    let updated = svc::update(
        &state.db,
        Actor::Human,
        &key,
        svc::UpdateNoteKind {
            display_name: payload.display_name,
            requires_approval: payload.requires_approval,
            description: payload.description,
            sort_order: payload.sort_order,
        },
    )
    .await?;
    Ok(Json(updated))
}

/// ノートが使用中の種別は削除しない
#[utoipa::path(
    delete,
    path = "/api/note-kinds/{key}",
    tag = "note_kinds",
    params(("key" = String, Path, description = "ノート種別 key")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既存のノートがこの種別を使用中", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_note_kind(
    State(state): State<AppState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    svc::delete(&state.db, Actor::Human, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}
