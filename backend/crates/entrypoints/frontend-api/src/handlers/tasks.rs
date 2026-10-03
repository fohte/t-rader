use axum::Json;
use axum::extract::State;
use core_application::strategy_task::TaskListQuery;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::handlers::strategies::map_list_task_error;
use crate::models::StrategyTaskSummary;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTasksQuery {
    pub strategy_id: Option<Uuid>,
    pub purpose: Option<String>,
}

/// 戦略タスクの実行履歴を口座横断で新しい順に一覧取得する。`strategy_id`/`purpose` で絞り込める。
#[utoipa::path(
    get,
    path = "/api/tasks",
    tag = "tasks",
    params(ListTasksQuery),
    responses(
        (status = 200, body = Vec<StrategyTaskSummary>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_tasks(
    State(state): State<FrontendApiState>,
    JsonQuery(p): JsonQuery<ListTasksQuery>,
) -> Result<Json<Vec<StrategyTaskSummary>>, AppError> {
    let views = state
        .strategy_task_use_cases
        .list(TaskListQuery {
            strategy_id: p.strategy_id,
            purpose: p.purpose,
        })
        .await
        .map_err(map_list_task_error)?;
    Ok(Json(
        views.into_iter().map(StrategyTaskSummary::from).collect(),
    ))
}
