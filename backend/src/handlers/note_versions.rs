use axum::Json;
use axum::extract::State;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::entities::{comment, note, note_version};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::handlers::strategies::map_submit_error;
use crate::models::ChangeStatusRequest;
use crate::services::change_history::{self, Op, TargetKind};
use crate::services::note_versions::{self, INITIAL_NOTE_STATUS};
use crate::services::strategy_tasks::{self, TaskSource};

async fn find_note_version<C: sea_orm::ConnectionTrait>(
    db: &C,
    note_id: Uuid,
    version_no: i32,
) -> Result<note_version::Model, AppError> {
    note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .filter(note_version::Column::VersionNo.eq(version_no))
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note version {note_id}/{version_no} not found")))
}

/// ノートの全バージョンを古い順に返す。
#[utoipa::path(
    get,
    path = "/api/notes/{id}/versions",
    tag = "notes",
    params(("id" = Uuid, Path, description = "ノート ID")),
    responses(
        (status = 200, body = Vec<note_version::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_note_versions(
    State(state): State<AppState>,
    JsonPath(note_id): JsonPath<Uuid>,
) -> Result<Json<Vec<note_version::Model>>, AppError> {
    if note::Entity::find_by_id(note_id)
        .one(&state.db)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!("note {note_id} not found")));
    }
    let versions = note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .order_by_asc(note_version::Column::VersionNo)
        .all(&state.db)
        .await?;
    Ok(Json(versions))
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
        (status = 200, body = note_version::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_note_version(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
) -> Result<Json<note_version::Model>, AppError> {
    Ok(Json(
        find_note_version(&state.db, note_id, version_no).await?,
    ))
}

/// 承認待ちバージョンを作成日時順に返す。
#[utoipa::path(
    get,
    path = "/api/note-versions/pending",
    tag = "notes",
    responses(
        (status = 200, body = Vec<note_version::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_pending_note_versions(
    State(state): State<AppState>,
) -> Result<Json<Vec<note_version::Model>>, AppError> {
    let versions = note_version::Entity::find()
        .filter(note_version::Column::Status.eq(INITIAL_NOTE_STATUS))
        .order_by_asc(note_version::Column::CreatedAt)
        .order_by_asc(note_version::Column::NoteId)
        .order_by_asc(note_version::Column::VersionNo)
        .all(&state.db)
        .await?;
    Ok(Json(versions))
}

fn ensure_pending_version(version: &note_version::Model) -> Result<(), AppError> {
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
        (status = 200, body = note_version::Model),
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
) -> Result<Json<note_version::Model>, AppError> {
    let txn = state.db.begin().await?;
    let version = find_note_version(&txn, note_id, version_no).await?;
    ensure_pending_version(&version)?;

    let now = chrono::Utc::now().fixed_offset();
    let (updated, previous_current_id) = note_versions::set_current_version(
        &txn,
        note_id,
        version.clone(),
        Some("approved"),
        Some(now),
    )
    .await?;
    change_history::record(
        &txn,
        TargetKind::Note,
        note_id,
        Op::StatusChange,
        json!({
            "from": version.status,
            "to": "approved",
            "version_id": version.id,
            "previous_current_version_id": previous_current_id,
            "label": payload.label.clone(),
        }),
        payload.label,
    )
    .await?;
    txn.commit().await?;
    Ok(Json(updated))
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
        (status = 200, body = note_version::Model),
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
) -> Result<Json<note_version::Model>, AppError> {
    let version = find_note_version(&state.db, note_id, version_no).await?;
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

    let note_row = note::Entity::find_by_id(note_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note {note_id} not found")))?;
    if let Some(strategy_id) = note_row.strategy_id {
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

    let txn = state.db.begin().await?;
    let current_version = find_note_version(&txn, note_id, version_no).await?;
    ensure_pending_version(&current_version)?;
    let now = chrono::Utc::now().fixed_offset();
    let updated = note_version::ActiveModel {
        id: Set(current_version.id),
        status: Set("rejected".to_string()),
        reviewed_at: Set(Some(now)),
        ..Default::default()
    }
    .update(&txn)
    .await?;
    note::ActiveModel {
        id: Set(note_id),
        updated_at: Set(now),
        ..Default::default()
    }
    .update(&txn)
    .await?;
    change_history::record(
        &txn,
        TargetKind::Note,
        note_id,
        Op::StatusChange,
        json!({
            "from": current_version.status,
            "to": "rejected",
            "version_id": current_version.id,
            "label": label.clone(),
        }),
        label,
    )
    .await?;
    txn.commit().await?;
    Ok(Json(updated))
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
        (status = 200, body = note_version::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn make_note_version_current(
    State(state): State<AppState>,
    JsonPath((note_id, version_no)): JsonPath<(Uuid, i32)>,
) -> Result<Json<note_version::Model>, AppError> {
    let txn = state.db.begin().await?;
    let version = find_note_version(&txn, note_id, version_no).await?;
    if version.status != "approved" {
        return Err(AppError::Conflict(format!(
            "note version {note_id}/{version_no} is not approved"
        )));
    }
    let current = note_versions::find_current_version(&txn, note_id).await?;
    if current
        .as_ref()
        .is_some_and(|current| current.id == version.id)
    {
        txn.commit().await?;
        return Ok(Json(version));
    }

    let (updated, previous_current_id) =
        note_versions::set_current_version(&txn, note_id, version.clone(), None, None).await?;
    change_history::record(
        &txn,
        TargetKind::Note,
        note_id,
        Op::Update,
        json!({
            "from_version_id": previous_current_id,
            "to_version_id": updated.id,
            "version_no": updated.version_no,
        }),
        None,
    )
    .await?;
    txn.commit().await?;
    Ok(Json(updated))
}
