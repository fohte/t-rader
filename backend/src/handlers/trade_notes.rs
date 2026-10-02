use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateTradeNoteRequest, NoteResponse, TradeNoteResponse};

/// 取引に紐づく判断ノート一覧 (リンク作成順。各ノートは紐付け時点で固定したバージョンを返す)
#[utoipa::path(
    get,
    path = "/api/trades/{id}/notes",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    responses(
        (status = 200, body = Vec<NoteResponse>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_trade_notes(
    State(state): State<AppState>,
    JsonPath(trade_id): JsonPath<Uuid>,
) -> Result<Json<Vec<NoteResponse>>, AppError> {
    let snapshots = state
        .trade_note_use_cases
        .list(trade_id)
        .await
        .map_err(super::trades::map_trade_error)?
        .into_iter()
        .map(NoteResponse::from_snapshot)
        .collect();
    Ok(Json(snapshots))
}

/// 取引に判断ノートを紐付ける (紐付け時点の現行バージョンを固定して記録する)
#[utoipa::path(
    post,
    path = "/api/trades/{id}/notes",
    tag = "trades",
    params(("id" = Uuid, Path, description = "取引 ID")),
    request_body = CreateTradeNoteRequest,
    responses(
        (status = 201, body = TradeNoteResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 409, description = "既にリンク済み", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_trade_note(
    State(state): State<AppState>,
    JsonPath(trade_id): JsonPath<Uuid>,
    JsonBody(p): JsonBody<CreateTradeNoteRequest>,
) -> Result<(StatusCode, Json<TradeNoteResponse>), AppError> {
    let created = state
        .trade_note_use_cases
        .create(trade_id, p.note_id)
        .await
        .map_err(super::trades::map_trade_error)?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

/// 取引と判断ノートの紐付けを解除する
#[utoipa::path(
    delete,
    path = "/api/trades/{id}/notes/{note_id}",
    tag = "trades",
    params(
        ("id" = Uuid, Path, description = "取引 ID"),
        ("note_id" = Uuid, Path, description = "ノート ID"),
    ),
    responses(
        (status = 204),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_trade_note(
    State(state): State<AppState>,
    JsonPath((trade_id, note_id)): JsonPath<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    state
        .trade_note_use_cases
        .delete(trade_id, note_id)
        .await
        .map_err(super::trades::map_trade_error)?;
    Ok(StatusCode::NO_CONTENT)
}
