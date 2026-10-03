use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use core_application::change_history::Actor;
use core_application::strategy::{
    CreateStrategyCommand, StrategyRepositoryError, StrategySummaryQueryError,
    StrategyUpdateCommand, StrategyUseCaseError,
};
use core_application::strategy_scope::{
    StrategyScope, StrategyScopeError, StrategyScopeSourceError,
};
use core_application::unit_of_work::UnitOfWorkError;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateStrategyRequest, StrategyResponse, UpdateStrategyRequest};

mod investable_amount;
mod tasks;

pub use investable_amount::{
    __path_get_investable_amount, __path_put_investable_amount, get_investable_amount,
    put_investable_amount,
};
pub use tasks::{
    __path_get_strategy_task, __path_list_strategy_tasks, __path_submit_strategy_chat,
    get_strategy_task, list_strategy_tasks, submit_strategy_chat,
};
pub(crate) use tasks::{map_list_task_error, map_submit_error};

pub(super) async fn strategy_scope_or_404(
    state: &FrontendApiState,
    id: Uuid,
) -> Result<StrategyScope, AppError> {
    state
        .strategy_scope_use_cases
        .verify(id)
        .await
        .map_err(|error| match error {
            StrategyScopeError::NotFound(id) => {
                AppError::NotFound(format!("strategy {id} not found"))
            }
            StrategyScopeError::Source(StrategyScopeSourceError::QueryFailed(message)) => {
                AppError::Internal(message)
            }
        })
}

pub(crate) fn map_strategy_error(error: StrategyUseCaseError) -> AppError {
    match error {
        StrategyUseCaseError::Validation(message) => AppError::Validation(message),
        StrategyUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        StrategyUseCaseError::Repository(StrategyRepositoryError::Database(error))
        | StrategyUseCaseError::SummaryQuery(StrategySummaryQueryError::Database(error))
        | StrategyUseCaseError::ChangeHistory(
            core_application::change_history::ChangeHistoryError::Database(error),
        )
        | StrategyUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | StrategyUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        other => AppError::Internal(other.to_string()),
    }
}

/// 戦略一覧
#[utoipa::path(
    get,
    path = "/api/strategies",
    tag = "strategies",
    responses(
        (status = 200, description = "戦略一覧", body = Vec<StrategyResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategies(
    State(state): State<FrontendApiState>,
) -> Result<Json<Vec<StrategyResponse>>, AppError> {
    let items = state
        .strategy_use_cases
        .list()
        .await
        .map_err(map_strategy_error)?;
    Ok(Json(
        items.into_iter().map(StrategyResponse::from).collect(),
    ))
}

/// 戦略取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = StrategyResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<StrategyResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let model = state
        .strategy_use_cases
        .get(scope)
        .await
        .map_err(map_strategy_error)?;
    Ok(Json(model.into()))
}

/// 戦略作成
#[utoipa::path(
    post,
    path = "/api/strategies",
    tag = "strategies",
    request_body = CreateStrategyRequest,
    responses(
        (status = 201, body = StrategyResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy(
    State(state): State<FrontendApiState>,
    JsonBody(payload): JsonBody<CreateStrategyRequest>,
) -> Result<(StatusCode, Json<StrategyResponse>), AppError> {
    let created = state
        .strategy_use_cases
        .create(
            Actor::Human,
            CreateStrategyCommand {
                name: payload.name,
                description: payload.description,
                sort_order: payload.sort_order.unwrap_or(0),
            },
        )
        .await
        .map_err(map_strategy_error)?;

    Ok((StatusCode::CREATED, Json(created.into())))
}

/// 戦略更新
#[utoipa::path(
    patch,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = UpdateStrategyRequest,
    responses(
        (status = 200, body = StrategyResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn update_strategy(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateStrategyRequest>,
) -> Result<Json<StrategyResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let updated = state
        .strategy_use_cases
        .update(
            Actor::Human,
            scope,
            StrategyUpdateCommand {
                name: payload.name,
                description: payload.description,
                sort_order: payload.sort_order,
            },
        )
        .await
        .map_err(map_strategy_error)?;

    Ok(Json(updated.into()))
}

/// 戦略削除
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 204),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn delete_strategy(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    state
        .strategy_use_cases
        .delete(Actor::Human, scope)
        .await
        .map_err(map_strategy_error)?;
    Ok(StatusCode::NO_CONTENT)
}
