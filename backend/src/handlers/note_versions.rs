use axum::Json;
use axum::extract::State;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::handlers::strategies::map_submit_error;
use crate::models::{ChangeStatusRequest, NoteVersionResponse};
use crate::services::note_versions::INITIAL_NOTE_STATUS;
use crate::services::strategy_tasks::{self, TaskSource};
use gateway_postgres::entities::comment;

/// ノートの全バージョンを古い順に返す。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/versions",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<NoteVersionResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_versions(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<NoteVersionResponse>>, AppError> {
    let versions = state
        .use_cases
        .note_reads()
        .list_note_versions(note_id)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

/// ノートの指定バージョンを返す。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/versions/{n}",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    Ok(Json(
        state
            .use_cases
            .note_reads()
            .get_note_version(note_id, version_no)
            .await
            .map_err(crate::handlers::notes::map_note_read_error)?
            .into(),
    ))
}

/// 承認待ちバージョンを作成日時順に返す。
#[utoipa::path(
    get,
    path = "/api/note-versions/pending",
    tag = "notes",
    responses(
        (status = 200, body = Vec<NoteVersionResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_pending_note_versions(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteVersionResponse>>, AppError> {
    let versions = state
        .use_cases
        .note_reads()
        .list_pending_note_versions()
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    Ok(Json(versions.into_iter().map(Into::into).collect()))
}

fn ensure_pending_version(version: &core_application::note::NoteVersion) -> Result<(), AppError> {
    if version.status != INITIAL_NOTE_STATUS {
        return Err(AppError::Conflict(format!(
            "note version {}/{} is not pending",
            version.note_id, version.version_no
        )));
    }
    Ok(())
}

/// 承認待ちバージョンを承認し、現行バージョンにする。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/versions/{n}/approve",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "バージョンが承認待ちではない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    let updated = state
        .use_cases
        .notes()
        .approve_version(note_id, version_no, payload.label)
        .await
        .map_err(crate::handlers::notes::map_note_error)?;
    Ok(Json(updated.into()))
}

/// 承認待ちバージョンを却下する。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/versions/{n}/reject",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正、または却下理由が必要", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "バージョンが承認待ちではない", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn reject_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    let version = state
        .use_cases
        .note_reads()
        .get_note_version(note_id, version_no)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    ensure_pending_version(&version)?;

    let line_comment_count = comment::Entity::find()
        .filter(comment::Column::TargetKind.eq("note_version"))
        .filter(comment::Column::TargetId.eq(version.id))
        .filter(comment::Column::ParentId.is_null())
        .filter(comment::Column::StartLine.is_not_null())
        .count(&state.db)
        .await?;
    let label = payload
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string);
    if line_comment_count == 0 && label.is_none() {
        return Err(AppError::Validation(
            "a rejection reason is required when no line comments are attached".into(),
        ));
    }

    let strategy_id = state
        .use_cases
        .note_reads()
        .get_note_strategy_id(note_id)
        .await
        .map_err(crate::handlers::notes::map_note_read_error)?;
    if let Some(strategy_id) = strategy_id {
        let reason = label
            .as_deref()
            .map(|label| format!("理由: {label}。"))
            .unwrap_or_default();
        let prompt = format!(
            "ノート「{}」(id: {}) の v{} (version_id: {}) がレビューで却下されました。{}付いているコメントを確認し、指摘を反映してください。",
            version.title, note_id, version_no, version.id, reason
        );
        strategy_tasks::submit_task(
            &state.db,
            &state.agent_task_client,
            strategy_id,
            &prompt,
            TaskSource::Review,
            None,
        )
        .await
        .map_err(map_submit_error)?;
    }

    let updated = state
        .use_cases
        .notes()
        .reject_version(note_id, version_no, label, line_comment_count > 0)
        .await
        .map_err(crate::handlers::notes::map_note_error)?;
    Ok(Json(updated.into()))
}

/// 承認済みの過去バージョンを現行にする。
#[utoipa::path(
    post,
    path = "/api/notes/{id}/versions/{n}/make-current",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        ("n" = i32, Path, description = "バージョン番号"),
    ),
    responses(
        (status = 200, body = NoteVersionResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn make_note_version_current(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
) -> Result<Json<NoteVersionResponse>, AppError> {
    let updated = state
        .use_cases
        .notes()
        .make_version_current(note_id, version_no)
        .await
        .map_err(crate::handlers::notes::map_note_error)?;
    Ok(Json(updated.into()))
}
