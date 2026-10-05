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
    NoteListQuery, NoteReadQueryError, NoteReadUseCaseError, NoteUseCaseError, NoteWriteCommand,
    UpdateNoteCommand,
};
use core_application::strategy_task_step_evidence::{
    StrategyTaskStepEvidenceRepositoryError, StrategyTaskStepEvidenceUseCaseError,
};
use core_application::unit_of_work::UnitOfWorkError;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListNotesQuery {
    pub status: Option<String>,
    pub kind: Option<String>,
    /// `frontmatter_json.tags` に完全一致するタグを持つノートだけを返す。
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetNoteQuery {
    /// 省略時は現行バージョンを返す。指定バージョンがこのノートに属さない場合は 404。
    pub version_id: Option<Uuid>,
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
        .list_notes(NoteListQuery {
            status: params.status.filter(|status| !status.is_empty()),
            kind: params.kind.filter(|kind| !kind.is_empty()),
            tag: params.tag.filter(|tag| !tag.is_empty()),
            limit: None,
            ..NoteListQuery::default()
        })
        .await
        .map_err(map_note_read_error)?;
    let responses = page
        .notes
        .into_iter()
        .map(NoteResponse::from_snapshot)
        .collect();
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
        .get_note(id, params.version_id, false)
        .await
        .map_err(map_note_read_error)?;
    Ok(Json(NoteResponse::from_snapshot(snapshot)))
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
    let history_title = payload.title.trim().to_string();
    let snapshot = state
        .note_use_cases
        .write(NoteWriteCommand {
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
            })),
        })
        .await
        .map_err(map_note_error)?;
    Ok((
        StatusCode::CREATED,
        Json(NoteResponse::from_snapshot(snapshot.snapshot)),
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
    Ok(Json(NoteResponse::from_snapshot(snapshot)))
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
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        NoteUseCaseError::StrategyTaskStepEvidence(
            StrategyTaskStepEvidenceUseCaseError::Repository(
                StrategyTaskStepEvidenceRepositoryError::Database(error),
            ),
        ) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}
