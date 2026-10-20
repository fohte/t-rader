use axum::Json;
use axum::extract::State;
use core_application::change_history::ChangeHistoryUseCaseError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonPath, JsonQuery};
use crate::models::ChangeHistoryResponse;

fn map_err(error: ChangeHistoryUseCaseError) -> AppError {
    match error {
        ChangeHistoryUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("history {id} not found"))
        }
        ChangeHistoryUseCaseError::InvalidCursor => {
            AppError::Validation("history cursor is invalid".into())
        }
        ChangeHistoryUseCaseError::Query(
            core_application::change_history::ChangeHistoryQueryError::Database(error),
        ) => error.into(),
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListHistoryQuery {
    pub target_kind: Option<String>,
    pub target_id: Option<Uuid>,
    /// 1〜500 (デフォルト 100)
    pub limit: Option<u64>,
    /// ページを続ける履歴 ID。target 条件を指定した場合は条件に一致する ID を指定する。
    pub before: Option<Uuid>,
}

/// 変更履歴一覧。target 条件がなければ全体の最新を返す。created_at と ID の降順で並べる。
#[utoipa::path(
    get,
    path = "/api/history",
    tag = "history",
    params(ListHistoryQuery),
    responses(
        (status = 200, body = Vec<ChangeHistoryResponse>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_history(
    State(state): State<FrontendApiState>,
    JsonQuery(p): JsonQuery<ListHistoryQuery>,
) -> Result<Json<Vec<ChangeHistoryResponse>>, AppError> {
    let entries = state
        .change_history_use_cases
        .list(p.target_kind.as_deref(), p.target_id, p.limit, p.before)
        .await
        .map_err(map_err)?;
    Ok(Json(
        entries
            .into_iter()
            .map(ChangeHistoryResponse::from)
            .collect(),
    ))
}

/// 変更履歴詳細
#[utoipa::path(
    get,
    path = "/api/history/{id}",
    tag = "history",
    params(("id" = Uuid, Path, description = "変更履歴 ID")),
    responses(
        (status = 200, body = ChangeHistoryResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_history(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<ChangeHistoryResponse>, AppError> {
    let entry = state
        .change_history_use_cases
        .get(id)
        .await
        .map_err(map_err)?;
    Ok(Json(ChangeHistoryResponse::from(entry)))
}
