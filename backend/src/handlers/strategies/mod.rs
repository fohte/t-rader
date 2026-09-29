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

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateStrategyRequest, StrategyResponse, UpdateStrategyRequest};
use gateway_postgres::PostgresStrategyScopeSource;

mod investable_amount;
mod tasks;

pub use investable_amount::{
    __path_get_investable_amount, __path_put_investable_amount, get_investable_amount,
    put_investable_amount,
};
pub(crate) use tasks::map_submit_error;
pub use tasks::{
    __path_get_strategy_task, __path_list_strategy_tasks, __path_submit_strategy_chat,
    get_strategy_task, list_strategy_tasks, submit_strategy_chat,
};

pub(super) async fn strategy_scope_or_404(
    state: &AppState,
    id: Uuid,
) -> Result<StrategyScope, AppError> {
    verify_strategy_scope(&state.db, id).await
}

pub(super) async fn find_strategy_or_404(
    db: &gateway_postgres::DatabaseHandle,
    id: Uuid,
) -> Result<(), AppError> {
    verify_strategy_scope(db, id).await.map(|_| ())
}

async fn verify_strategy_scope(
    db: &gateway_postgres::DatabaseHandle,
    id: Uuid,
) -> Result<StrategyScope, AppError> {
    let source = PostgresStrategyScopeSource::new(db);
    StrategyScope::verify(id, &source)
        .await
        .map_err(|error| match error {
            StrategyScopeError::NotFound(id) => {
                AppError::NotFound(format!("strategy {id} not found"))
            }
            StrategyScopeError::Source(StrategyScopeSourceError::QueryFailed(message)) => {
                AppError::Database(sea_orm::DbErr::Custom(message))
            }
        })
}

pub(crate) fn map_strategy_error(error: StrategyUseCaseError) -> AppError {
    match error {
        StrategyUseCaseError::Validation(message) => AppError::Validation(message),
        StrategyUseCaseError::NotFound(id) => {
            AppError::NotFound(format!("strategy {id} not found"))
        }
        StrategyUseCaseError::ConfirmationMismatch(id) => AppError::NotFound(format!(
            "strategy {id} not found or name changed since confirmation"
        )),
        StrategyUseCaseError::Repository(StrategyRepositoryError::Database(error))
        | StrategyUseCaseError::SummaryQuery(StrategySummaryQueryError::Database(error))
        | StrategyUseCaseError::ChangeHistory(
            core_application::change_history::ChangeHistoryError::Database(error),
        )
        | StrategyUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | StrategyUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => error.into(),
        other => AppError::Database(sea_orm::DbErr::Custom(other.to_string())),
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
    State(state): State<AppState>,
) -> Result<Json<Vec<StrategyResponse>>, AppError> {
    let items = state
        .use_cases
        .strategies
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
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<StrategyResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let model = state
        .use_cases
        .strategies
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
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateStrategyRequest>,
) -> Result<(StatusCode, Json<StrategyResponse>), AppError> {
    let created = state
        .use_cases
        .strategies
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
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<UpdateStrategyRequest>,
) -> Result<Json<StrategyResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let updated = state
        .use_cases
        .strategies
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
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<StatusCode, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    state
        .use_cases
        .strategies
        .delete(Actor::Human, scope)
        .await
        .map_err(map_strategy_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::testing::{create_strategy, create_test_server};
    use serde_json::json;

    #[backend_test_macros::database_test]
    async fn create_and_list_strategy(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .post("/api/strategies")
            .json(&json!({ "name": "長期投資" }))
            .await;
        res.assert_status(axum::http::StatusCode::CREATED);

        let list = server.get("/api/strategies").await;
        list.assert_status_ok();
        let body: Vec<serde_json::Value> = list.json();
        assert_eq!(body.len(), 1);
        assert_eq!(body[0]["name"], "長期投資");
    }

    #[backend_test_macros::database_test]
    async fn get_nonexistent_strategy_returns_404(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[backend_test_macros::database_test]
    async fn delete_strategy_removes_row(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let id = create_strategy(&server, "to-delete").await;

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        deleted.assert_status(axum::http::StatusCode::NO_CONTENT);

        let get = server.get(&format!("/api/strategies/{id}")).await;
        get.assert_status(axum::http::StatusCode::NOT_FOUND);
    }
}
