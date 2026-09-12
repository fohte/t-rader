use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::AppState;
use crate::entities::{strategy, strategy_interest};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{CreateStrategyRequest, UpdateStrategyRequest};
use crate::services::change_history::Actor;
use crate::services::strategy_config;

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

pub(super) use strategy_config::find_or_404 as find_strategy_or_404;

/// 戦略一覧
#[utoipa::path(
    get,
    path = "/api/strategies",
    tag = "strategies",
    responses(
        (status = 200, description = "戦略一覧", body = Vec<strategy::Model>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategies(
    State(state): State<AppState>,
) -> Result<Json<Vec<strategy::Model>>, AppError> {
    let items = strategy::Entity::find()
        .order_by_asc(strategy::Column::SortOrder)
        .order_by_asc(strategy::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

/// 戦略取得
#[utoipa::path(
    get,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_strategy(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<strategy::Model>, AppError> {
    let model = find_strategy_or_404(&state.db, id).await?;
    Ok(Json(model))
}

/// 戦略作成
#[utoipa::path(
    post,
    path = "/api/strategies",
    tag = "strategies",
    request_body = CreateStrategyRequest,
    responses(
        (status = 201, body = strategy::Model),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, description = "リクエストボディのパースに失敗", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn create_strategy(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<CreateStrategyRequest>,
) -> Result<(StatusCode, Json<strategy::Model>), AppError> {
    let created = strategy_config::create(
        &state.db,
        Actor::Human,
        strategy_config::CreateStrategy {
            name: payload.name,
            description: payload.description,
            sort_order: payload.sort_order.unwrap_or(0),
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(created)))
}

/// 戦略更新
#[utoipa::path(
    patch,
    path = "/api/strategies/{id}",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = UpdateStrategyRequest,
    responses(
        (status = 200, body = strategy::Model),
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
) -> Result<Json<strategy::Model>, AppError> {
    let updated = strategy_config::update(
        &state.db,
        Actor::Human,
        id,
        strategy_config::StrategyUpdate {
            name: payload.name,
            description: payload.description,
            sort_order: payload.sort_order,
        },
    )
    .await?;

    Ok(Json(updated))
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
    find_strategy_or_404(&state.db, id).await?;
    strategy_config::delete(&state.db, Actor::Human, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 戦略の関心 (シード + LLM 派生) 一覧
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/interests",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = Vec<strategy_interest::Model>),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_interests(
    State(state): State<AppState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<Vec<strategy_interest::Model>>, AppError> {
    find_strategy_or_404(&state.db, id).await?;
    let items = strategy_interest::Entity::find()
        .filter(strategy_interest::Column::StrategyId.eq(id))
        .order_by_asc(strategy_interest::Column::Role)
        .order_by_asc(strategy_interest::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(items))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testing::{create_strategy, create_test_server};

    #[sqlx::test(migrations = false)]
    async fn create_and_list_strategy(pool: PgPool) {
        let server = create_test_server(pool).await;
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

    #[sqlx::test(migrations = false)]
    async fn get_nonexistent_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_interests_of_nonexistent_strategy_returns_404(pool: PgPool) {
        let server = create_test_server(pool).await;
        let res = server
            .get("/api/strategies/00000000-0000-0000-0000-000000000000/interests")
            .await;
        res.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn delete_strategy_removes_row(pool: PgPool) {
        let server = create_test_server(pool).await;
        let id = create_strategy(&server, "to-delete").await;

        let deleted = server.delete(&format!("/api/strategies/{id}")).await;
        deleted.assert_status(axum::http::StatusCode::NO_CONTENT);

        let get = server.get(&format!("/api/strategies/{id}")).await;
        get.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_interests_of_existing_strategy_returns_empty(pool: PgPool) {
        let server = create_test_server(pool).await;
        let created = server
            .post("/api/strategies")
            .json(&json!({ "name": "test" }))
            .await;
        created.assert_status(axum::http::StatusCode::CREATED);
        let id = created.json::<serde_json::Value>()["id"]
            .as_str()
            .map(str::to_string)
            .expect("id");

        let res = server.get(&format!("/api/strategies/{id}/interests")).await;
        res.assert_status_ok();
        let body: Vec<serde_json::Value> = res.json();
        assert!(body.is_empty());
    }
}
