//! ノートタグ語彙 CRUD の REST handler。

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::Actor;
use core_application::note_tag::{CreateNoteTagCommand, UpdateNoteTagCommand};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::NoteTagResponse;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateNoteTagRequest {
    pub key: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateNoteTagRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::serde_helpers::deserialize_nullable_option"
    )]
    pub description: Option<Option<String>>,
}

/// ノートタグ一覧
#[utoipa::path(
    get,
    path = "/api/note-tags",
    tag = "note_tags",
    responses(
        (status = 200, body = Vec<NoteTagResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_tags(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<NoteTagResponse>>, AppError> {
    Ok(Json(
        state
            .note_tag_use_cases
            .list()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// ノートタグを作成
#[utoipa::path(
    post,
    path = "/api/note-tags",
    tag = "note_tags",
    request_body = CreateNoteTagRequest,
    responses(
        (status = 201, body = NoteTagResponse),
        (status = 400, body = ErrorResponse),
        (status = 409, description = "key が既存と衝突", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note_tag(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateNoteTagRequest>,
) -> Result<(StatusCode, Json<NoteTagResponse>), AppError> {
    let created = state
        .note_tag_use_cases
        .create(
            Actor::Human,
            CreateNoteTagCommand {
                key: payload.key,
                display_name: payload.display_name,
                description: payload.description,
            },
        )
        .await?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// ノートタグを部分更新する (key は変更不可)
#[utoipa::path(
    patch,
    path = "/api/note-tags/{key}",
    tag = "note_tags",
    params(("key" = String, Path, description = "ノートタグ key")),
    request_body = UpdateNoteTagRequest,
    responses(
        (status = 200, body = NoteTagResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note_tag(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
    JsonBody(payload): JsonBody<UpdateNoteTagRequest>,
) -> Result<Json<NoteTagResponse>, AppError> {
    let updated = state
        .note_tag_use_cases
        .update(
            Actor::Human,
            &key,
            UpdateNoteTagCommand {
                display_name: payload.display_name,
                description: payload.description,
            },
        )
        .await?;
    Ok(Json(updated.into()))
}

/// ノートタグを削除
#[utoipa::path(
    delete,
    path = "/api/note-tags/{key}",
    tag = "note_tags",
    params(("key" = String, Path, description = "ノートタグ key")),
    responses(
        (status = 204),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_note_tag(
    State(state): State<FrontendApiState>,
    JsonPath(key): JsonPath<String>,
) -> Result<StatusCode, AppError> {
    state.note_tag_use_cases.delete(Actor::Human, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}
