//! ノート種別 CRUD の REST handler。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::Actor;
use core_application::note_kind::{CreateNoteKindCommand, UpdateNoteKindCommand};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::NoteKindResponse;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct UpdateNoteKindRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub requires_approval: Option<bool>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// ノート種別一覧
#[utoipa::path(
    get,
    path = "/api/note-kinds",
    tag = "note_kinds",
    responses(
        (status = 200, body = Vec<NoteKindResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_kinds(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<NoteKindResponse>>, AppError> {
    Ok(Json(
        state
            .note_kind_use_cases
            .list()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// ノート種別を作成
#[utoipa::path(
    post,
    path = "/api/note-kinds",
    tag = "note_kinds",
    request_body = CreateNoteKindRequest,
    responses(
        (status = 201, body = NoteKindResponse),
        (status = 400, body = ErrorResponse),
        (status = 409, description = "key が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note_kind(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateNoteKindRequest>,
) -> Result<(StatusCode, Json<NoteKindResponse>), AppError> {
    let created = state
        .note_kind_use_cases
        .create(
            Actor::Human,
            CreateNoteKindCommand {
                key: payload.key,
                display_name: payload.display_name,
                requires_approval: payload.requires_approval,
                description: payload.description,
                sort_order: payload.sort_order,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// ノート種別を部分更新する (key は変更不可)
#[utoipa::path(
    patch,
    path = "/api/note-kinds/{key}",
    tag = "note_kinds",
    params(("key" = String, Path, description = "ノート種別 key")),
    request_body = UpdateNoteKindRequest,
    responses(
        (status = 200, body = NoteKindResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note_kind(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateNoteKindRequest>,
) -> Result<Json<NoteKindResponse>, AppError> {
    let updated = state
        .note_kind_use_cases
        .update(
            Actor::Human,
            &key,
            UpdateNoteKindCommand {
                display_name: payload.display_name,
                requires_approval: payload.requires_approval,
                description: payload.description,
                sort_order: payload.sort_order,
            },
        )
        .await?;
    Ok(Json(updated.into()))
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
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state.note_kind_use_cases.delete(Actor::Human, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}
