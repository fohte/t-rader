use axum::Json;
use axum::extract::State;
use chrono::Utc;
use core_application::change_history::Actor;
use core_application::strategy::InvestableAmount;
use uuid::Uuid;

use super::{map_strategy_error, strategy_scope_or_404};
use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::{JsonBody, JsonPath};
use crate::models::{InvestableAmountResponse, PutInvestableAmountRequest};

fn to_response(current: Option<InvestableAmount>) -> InvestableAmountResponse {
    InvestableAmountResponse {
        amount_jpy: current.as_ref().map(|m| m.amount_jpy),
        effective_at: current.map(|m| m.effective_at),
    }
}

/// 戦略の投資可能額 (`effective_at` が現在時刻以下の最新行) を取得。
/// history が無い戦略では `amount_jpy` / `effective_at` ともに null を返す。
#[utoipa::path(
    get,
    path = "/api/strategies/{id}/investable-amount",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    responses(
        (status = 200, body = InvestableAmountResponse),
        (status = 400, description = "リクエストパラメータが不正", body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_investable_amount(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
) -> Result<Json<InvestableAmountResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let current = state
        .strategy_use_cases
        .current_investable_amount(scope)
        .await
        .map_err(map_strategy_error)?;
    Ok(Json(to_response(current)))
}

/// 戦略の投資可能額を新しい history 行として記録する。既存行は上書きしない。
#[utoipa::path(
    put,
    path = "/api/strategies/{id}/investable-amount",
    tag = "strategies",
    params(("id" = Uuid, Path, description = "戦略 ID")),
    request_body = PutInvestableAmountRequest,
    responses(
        (status = 200, body = InvestableAmountResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_investable_amount(
    State(state): State<FrontendApiState>,
    JsonPath(id): JsonPath<Uuid>,
    JsonBody(payload): JsonBody<PutInvestableAmountRequest>,
) -> Result<Json<InvestableAmountResponse>, AppError> {
    let scope = strategy_scope_or_404(&state, id).await?;
    let effective_at = payload
        .effective_at
        .unwrap_or_else(|| Utc::now().fixed_offset());
    let created = state
        .strategy_use_cases
        .record_investable_amount(Actor::Human, scope, payload.amount_jpy, effective_at)
        .await
        .map_err(map_strategy_error)?;
    Ok(Json(to_response(Some(created))))
}
