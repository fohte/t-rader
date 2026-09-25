use axum::Json;
use axum::extract::State;
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::Set;
use sea_orm::TransactionTrait;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{note, note_version};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::handlers::strategies::map_submit_error;
use crate::models::{ChangeStatusRequest, NoteResponse};
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::strategy_tasks::{self, TaskSource};

use super::{CurrentNote, current_note_response, find_current_note_or_404};

async fn change_note_status_from(
    state: &AppState,
    current: CurrentNote,
    new_status: &str,
    label: Option<String>,
) -> Result<NoteResponse, AppError> {
    let CurrentNote {
        note: current_note,
        version: current_version,
        created_by_kind,
    } = current;
    let id = current_note.id;
    let updated_at = chrono::Utc::now().fixed_offset();
    let txn = state.db.begin().await?;
    let updated_version = note_version::ActiveModel {
        id: Set(current_version.id),
        status: Set(new_status.to_string()),
        reviewed_at: Set(Some(updated_at)),
        ..Default::default()
    }
    .update(&txn)
    .await?;
    let updated_note = note::ActiveModel {
        id: Set(id),
        updated_at: Set(updated_at),
        ..Default::default()
    }
    .update(&txn)
    .await?;
    change_history::record(
        &txn,
        TargetKind::Note,
        id,
        Op::StatusChange,
        json!({
            "from": current_version.status,
            "to": new_status,
            "version_id": current_version.id,
            "label": label,
        }),
        label,
    )
    .await?;
    txn.commit().await?;
    Ok(NoteResponse::from_current_version(
        updated_note,
        updated_version,
        created_by_kind,
    ))
}

async fn change_note_status(
    state: &AppState,
    id: Uuid,
    new_status: &str,
    label: Option<String>,
) -> Result<NoteResponse, AppError> {
    let current = find_current_note_or_404(&state.db, id).await?;
    change_note_status_from(state, current, new_status, label).await
}

/// ノートを approved に遷移
#[utoipa::path(
    post,
    path = "/api/notes/{id}/approve",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn approve_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<NoteResponse>, AppError> {
    Ok(Json(
        change_note_status(&state, id, "approved", payload.label).await?,
    ))
}

/// ノートを rejected に遷移
#[utoipa::path(
    post,
    path = "/api/notes/{id}/reject",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    request_body = ChangeStatusRequest,
    responses(
        (status = 200, body = NoteResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
        (status = 503, description = "agent task client が未設定、または agent_config が見つからない", body = ErrorResponse),
    )
)]
pub async fn reject_note(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<ChangeStatusRequest>,
) -> Result<Json<NoteResponse>, AppError> {
    let current = find_current_note_or_404(&state.db, id).await?;
    // 却下確定前の check-then-act。ほぼ同時に reject が 2 回届くと両方通過し得るが、
    // frontend は mutation pending 中ボタンを disable するため実運用では起きない。
    if current.version.status == "rejected" {
        return Ok(Json(current_note_response(current)));
    }

    if let Some(strategy_id) = current.note.strategy_id {
        let prompt = format!(
            "ノート「{}」(id: {}) がレビューで却下されました。付いているコメントを確認し、指摘を反映してください。",
            current.version.title, current.note.id
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

    Ok(Json(
        change_note_status_from(&state, current, "rejected", payload.label).await?,
    ))
}
