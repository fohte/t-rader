use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use core_domain::financial_summary::FinancialSummaryPeriodType;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonPath;
use crate::extractors::JsonQuery;
use crate::models::{
    StockFinancialSummaryResponse, StockGroupMembershipResponse, StockNextEarningsResponse,
    StockValuationResponse,
};

/// 銘柄の所属グループ
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/groups",
    tag = "stocks",
    params(("stock_id" = String, Path, description = "銘柄 ID")),
    responses(
        (status = 200, body = Vec<StockGroupMembershipResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_stock_groups(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
) -> Result<Json<Vec<StockGroupMembershipResponse>>, AppError> {
    let memberships = state
        .stock_group_use_cases
        .list_memberships(&[stock_id], None)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(memberships.into_iter().map(Into::into).collect()))
}

/// 銘柄の次の決算予定
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/next-earnings",
    tag = "stocks",
    params(("stock_id" = String, Path, description = "銘柄 ID")),
    responses(
        (status = 200, body = Option<StockNextEarningsResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_stock_next_earnings(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
) -> Result<Json<Option<StockNextEarningsResponse>>, AppError> {
    let event = state
        .calendar_event_read_use_cases
        .find_next_earnings_for_stock(&stock_id)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(event.map(Into::into)))
}

/// 銘柄の最新バリュエーション
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/valuation",
    tag = "stocks",
    params(("stock_id" = String, Path, description = "銘柄 ID")),
    responses(
        (status = 200, body = Option<StockValuationResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_stock_valuation(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
) -> Result<Json<Option<StockValuationResponse>>, AppError> {
    let valuation = state
        .valuation_use_cases
        .find_latest_for_symbol(&stock_id)
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(valuation.map(Into::into)))
}

/// 銘柄の決算短信検索条件
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StockFinancialSummaryQuery {
    /// 対象期末。期区分と同時に指定する
    pub current_period_end: Option<NaiveDate>,
    /// 対象期区分。期末と同時に指定する
    pub current_period_type: Option<String>,
}

/// 銘柄の決算短信要約一覧
#[utoipa::path(
    get,
    path = "/api/stocks/{stock_id}/financial-summaries",
    tag = "stocks",
    params(
        ("stock_id" = String, Path, description = "銘柄 ID"),
        StockFinancialSummaryQuery,
    ),
    responses(
        (status = 200, body = Vec<StockFinancialSummaryResponse>),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_stock_financial_summaries(
    State(state): State<FrontendApiState>,
    JsonPath(stock_id): JsonPath<String>,
    JsonQuery(params): JsonQuery<StockFinancialSummaryQuery>,
) -> Result<Json<Vec<StockFinancialSummaryResponse>>, AppError> {
    let summaries = match (params.current_period_end, params.current_period_type) {
        (None, None) => {
            state
                .financial_summary_use_cases
                .list_for_symbol(&stock_id)
                .await
        }
        (Some(period_end), Some(period_type)) => {
            let Some(period_type) = FinancialSummaryPeriodType::parse(&period_type) else {
                return Err(AppError::Validation(
                    "current_period_type must be FY, 1Q, 2Q, 3Q, or 4Q".into(),
                ));
            };
            state
                .financial_summary_use_cases
                .list_for_period(&stock_id, period_end, period_type)
                .await
        }
        _ => {
            return Err(AppError::Validation(
                "current_period_end and current_period_type must be provided together".into(),
            ));
        }
    }
    .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(Json(summaries.into_iter().map(Into::into).collect()))
}
