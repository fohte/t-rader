use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath, JsonQuery};
use crate::models::{CreateNoteRequest, NoteResponse, UpdateNoteRequest};
use core_application::change_history::{Actor, ChangeHistoryError};
use core_application::note::NoteRepositoryError;
use core_application::note::{
    NoteListQuery, NoteReadQueryError, NoteReadUseCaseError, NoteSnapshot, NoteUseCaseError,
    NoteWriteCommand, UpdateNoteCommand,
};
use core_application::strategy_existence::StrategyExistenceError;
use core_application::unit_of_work::UnitOfWorkError;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListNotesQuery {
    pub strategy_id: Option<Uuid>,
    pub status: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetNoteQuery {
    /// 省略時は現行バージョンを返す。指定バージョンがこのノートに属さない場合は 404。
    pub version_id: Option<Uuid>,
}

fn note_snapshot_response(snapshot: NoteSnapshot) -> NoteResponse {
    NoteResponse {
        id: snapshot.note.id,
        version_id: snapshot.version.id,
        version_no: snapshot.version.version_no,
        is_current: snapshot.version.is_current,
        strategy_id: snapshot.note.strategy_id,
        title: snapshot.version.title,
        body_md: snapshot.version.body_md,
        frontmatter_json: snapshot.version.frontmatter_json,
        kind: snapshot.note.kind,
        status: snapshot.version.status,
        trigger: snapshot.note.trigger,
        trigger_label: snapshot.note.trigger_label,
        created_by_kind: snapshot.created_by_kind,
        created_at: snapshot.note.created_at,
        updated_at: snapshot.note.updated_at,
        graphs_json: snapshot.version.graphs_json,
        execution_id: snapshot.note.execution_id,
    }
}

/// ノート一覧
#[utoipa::path(
    get,
    path = "/api/notes",
    tag = "notes",
    params(ListNotesQuery),
    responses(
        (status = 200, body = Vec<NoteResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_notes(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<ListNotesQuery>,
) -> Result<Json<Vec<NoteResponse>>, AppError> {
    let page = state
        .note_read_use_cases
        .list_notes(
            None,
            NoteListQuery {
                strategy_id: params.strategy_id,
                status: params.status.filter(|status| !status.is_empty()),
                kind: params.kind.filter(|kind| !kind.is_empty()),
                limit: None,
                ..NoteListQuery::default()
            },
        )
        .await
        .map_err(map_note_read_error)?;
    let responses = page.notes.into_iter().map(note_snapshot_response).collect();
    Ok(Json(responses))
}

/// ノート取得
#[utoipa::path(
    get,
    path = "/api/notes/{id}",
    tag = "notes",
    params(
        ("id" = Uuid, Path, description = "ノート ID"),
        GetNoteQuery,
    ),
    responses(
        (status = 200, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonQuery(params): JsonQuery<GetNoteQuery>,
) -> Result<Json<NoteResponse>, AppError> {
    let snapshot = state
        .note_read_use_cases
        .get_note(id, params.version_id, false, None)
        .await
        .map_err(map_note_read_error)?;
    Ok(Json(note_snapshot_response(snapshot)))
}

pub(super) fn map_note_read_error(error: NoteReadUseCaseError) -> AppError {
    match error {
        NoteReadUseCaseError::NotFound(message) => AppError::NotFound(message),
        NoteReadUseCaseError::VersionDoesNotBelong {
            note_id,
            version_id,
        } => AppError::NotFound(format!("version {version_id} for note {note_id} not found")),
        NoteReadUseCaseError::NoVersion(note_id) => {
            AppError::NotFound(format!("current version for note {note_id} not found"))
        }
        NoteReadUseCaseError::InitialVersionNotFound(note_id) => {
            AppError::NotFound(format!("initial version for note {note_id} not found"))
        }
        NoteReadUseCaseError::VersionNumberNotFound {
            note_id,
            version_no,
        } => AppError::NotFound(format!("note version {note_id}/{version_no} not found")),
        NoteReadUseCaseError::NoteVersionNotFound => {
            AppError::NotFound("note version not found".into())
        }
        NoteReadUseCaseError::Query(NoteReadQueryError::Database(error)) => error.into(),
        NoteReadUseCaseError::Query(NoteReadQueryError::InvalidData(message)) => {
            AppError::Internal(message)
        }
    }
}

/// ノート作成
#[utoipa::path(
    post,
    path = "/api/notes",
    tag = "notes",
    request_body = CreateNoteRequest,
    responses(
        (status = 201, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_note(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), AppError> {
    let created_by_kind = payload.created_by_kind.unwrap_or_else(|| "human".into());
    let strategy_id = payload.strategy_id;
    let history_title = payload.title.trim().to_string();
    let snapshot = state
        .note_use_cases
        .write(NoteWriteCommand {
            scope: None,
            strategy_id,
            execution_id: None,
            note_id: None,
            title: Some(payload.title),
            body_md: Some(payload.body_md),
            frontmatter_json: Some(
                payload
                    .frontmatter_json
                    .unwrap_or_else(|| serde_json::json!({})),
            ),
            graphs_json: Some(serde_json::json!([])),
            kind: payload.kind.map(Some),
            status: payload.status,
            trigger: payload.trigger.map(|trigger| trigger.to_string()),
            trigger_label: payload.trigger_label,
            created_by_kind,
            change_reason: None,
            actor: Actor::Human,
            change_diff: Some(serde_json::json!({
                "title": history_title,
                "strategy_id": strategy_id,
            })),
        })
        .await
        .map_err(map_note_error)?;
    Ok((
        StatusCode::CREATED,
        Json(note_snapshot_response(snapshot.snapshot)),
    ))
}

/// ノート更新
#[utoipa::path(
    patch,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = UpdateNoteRequest,
    responses(
        (status = 200, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_note(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateNoteRequest>,
) -> Result<Json<NoteResponse>, AppError> {
    let snapshot = state
        .note_use_cases
        .update(
            id,
            UpdateNoteCommand {
                title: payload.title,
                body_md: payload.body_md,
                frontmatter_json: payload.frontmatter_json,
                kind: payload.kind,
                trigger: payload.trigger.map(|trigger| trigger.to_string()),
                trigger_label: payload.trigger_label,
            },
        )
        .await
        .map_err(map_note_error)?;
    Ok(Json(note_snapshot_response(snapshot)))
}

/// ノート削除
#[utoipa::path(
    delete,
    path = "/api/notes/{id}",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_note(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    state
        .note_use_cases
        .delete(id)
        .await
        .map_err(map_note_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn map_note_error(error: NoteUseCaseError) -> AppError {
    match error {
        NoteUseCaseError::Validation(message) => AppError::Validation(message),
        NoteUseCaseError::UnknownNoteKind(kind) => {
            AppError::Validation(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::ReferencedNoteKindNotFound(kind) => {
            AppError::NotFound(format!("note kind {kind} not found"))
        }
        NoteUseCaseError::NotFound(message) => AppError::NotFound(message),
        NoteUseCaseError::Conflict(message) => AppError::Conflict(message),
        NoteUseCaseError::Repository(NoteRepositoryError::Database(error))
        | NoteUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error))
        | NoteUseCaseError::StrategyExistence(StrategyExistenceError::Database(error)) => {
            error.into()
        }
        other => AppError::Internal(other.to_string()),
    }
}
