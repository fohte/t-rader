use axum::Json;
use axum::extract::State;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::handlers::strategies::map_get_task_error;
use crate::models::StrategyTaskStepOriginResponse;

/// 実行ステップが属する戦略と実行を取得する
#[utoipa::path(
    get,
    path = "/api/strategy-task-steps/{step_id}",
    tag = "tasks",
    operation_id = "get_strategy_task_step_origin",
    params(("step_id" = Uuid, Path, description = "実行ステップ ID")),
    responses(
        (status = 200, body = StrategyTaskStepOriginResponse),
        (status = 400, description = "実行ステップ ID が不正", body = ErrorResponse),
        (status = 404, description = "実行ステップまたは親の実行が存在しない", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy_task_step_origin(
    State(state): State<FrontendApiState>,
    JsonPath(step_id): JsonPath<Uuid>,
) -> Result<Json<StrategyTaskStepOriginResponse>, AppError> {
    let origin = state
        .strategy_task_use_cases
        .find_origin_by_execution_step_id(step_id)
        .await
        .map_err(map_get_task_error)?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "strategy task origin for execution step {step_id} not found"
            ))
        })?;

    Ok(Json(StrategyTaskStepOriginResponse::from(origin)))
}
