use axum::Json;
use axum::extract::{Query, State};
use core_application::strategy_earnings_target::StrategyEarningsTargetUseCaseError;
use uuid::Uuid;

use super::strategy_scope_or_404;
use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{
    AddStrategyEarningsTargetRequest, RemoveStrategyEarningsTargetQuery,
    StrategyEarningsTargetChangeResponse, StrategyEarningsTargetResponse,
};

fn map_earnings_target_error(error: StrategyEarningsTargetUseCaseError) -> AppError {
    match error {
        StrategyEarningsTargetUseCaseError::Validation(message)
        | StrategyEarningsTargetUseCaseError::Reference(
            core_application::refs::RefUseCaseError::Validation(message),
        ) => AppError::Validation(message),
        error @ StrategyEarningsTargetUseCaseError::ReferenceNotFound { .. } => {
            AppError::Validation(error.to_string())
        }
        error => AppError::Internal(error.to_string()),
    }
}

/// 戦略が決算を追う銘柄とグループを一覧する。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/earnings-targets",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, description = "決算対象の一覧", body = Vec<StrategyEarningsTargetResponse>),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_strategy_earnings_targets(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<Vec<StrategyEarningsTargetResponse>>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let targets = state
        .strategy_earnings_target_use_cases
        .list(scope)
        .await
        .map_err(map_earnings_target_error)?;
    Ok(Json(
        targets
            .into_iter()
            .map(StrategyEarningsTargetResponse::from)
            .collect(),
    ))
}

/// 戦略が決算を追う銘柄またはグループを追加する。
#[utoipa::path(
    post,
    path = "/api/strategies/{id}/earnings-targets",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = AddStrategyEarningsTargetRequest,
    responses(
        (status = 200, description = "追加されたかどうか", body = StrategyEarningsTargetChangeResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn add_strategy_earnings_target(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<AddStrategyEarningsTargetRequest>,
) -> Result<Json<StrategyEarningsTargetChangeResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let changed = state
        .strategy_earnings_target_use_cases
        .add(scope, &payload.ref_kind, &payload.ref_id)
        .await
        .map_err(map_earnings_target_error)?;
    Ok(Json(StrategyEarningsTargetChangeResponse { changed }))
}

/// 戦略が決算を追う銘柄またはグループを削除する。
#[utoipa::path(
    delete,
    path = "/api/strategies/{id}/earnings-targets",
    tag = "strategies",
    params(
        ("id" = Uuid, Path, description = "戦略 ID"),
        ("ref_kind" = String, Query, description = "参照型"),
        ("ref_id" = String, Query, description = "銘柄またはグループの ID")
    ),
    responses(
        (status = 200, description = "削除されたかどうか", body = StrategyEarningsTargetChangeResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn remove_strategy_earnings_target(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    Query(query): Query<RemoveStrategyEarningsTargetQuery>,
) -> Result<Json<StrategyEarningsTargetChangeResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let changed = state
        .strategy_earnings_target_use_cases
        .remove(scope, &query.ref_kind, &query.ref_id)
        .await
        .map_err(map_earnings_target_error)?;
    Ok(Json(StrategyEarningsTargetChangeResponse { changed }))
}
