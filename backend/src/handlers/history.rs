use axum::Json;
use axum::extract::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::AppState;
use crate::entities::change_history;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListHistoryQuery {
    pub target_kind: Option<String>,
    pub target_id: Option<Uuid>,
    /// 1〜500 (デフォルト 100)
    pub limit: Option<u64>,
}

/// 変更履歴一覧。target で絞らない場合は全体の最新を返す。
#[utoipa::path(
    get,
    path = "/api/history",
    tag = "history",
    params(ListHistoryQuery),
    responses(
        (status = 200, body = Vec<change_history::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_history(
    State(state): State<AppState>,
    JsonQuery(p): JsonQuery<ListHistoryQuery>,
) -> Result<Json<Vec<change_history::Model>>, AppError> {
    let mut q = change_history::Entity::find().order_by_desc(change_history::Column::CreatedAt);
    if let Some(kind) = p.target_kind.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(change_history::Column::TargetKind.eq(kind));
    }
    if let Some(tid) = p.target_id {
        q = q.filter(change_history::Column::TargetId.eq(tid));
    }
    let limit = p.limit.unwrap_or(100).clamp(1, 500);
    Ok(Json(q.limit(limit).all(&state.db).await?))
}

/// 変更履歴詳細
#[utoipa::path(
    get,
    path = "/api/history/{id}",
    tag = "history",
    params(("id" = Uuid, Path, description = "変更履歴 ID")),
    responses(
        (status = 200, body = change_history::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_history(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<change_history::Model>, AppError> {
    let m = change_history::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("history {id} not found")))?;
    Ok(Json(m))
}
