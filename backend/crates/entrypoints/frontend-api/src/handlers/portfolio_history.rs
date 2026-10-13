use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use core_application::portfolio_history::PortfolioHistoryQuery;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::models::PortfolioHistoryPointResponse;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PortfolioHistoryParams {
    /// 取得開始日 (両端を含む)
    pub from: NaiveDate,
    /// 取得終了日 (両端を含む)
    pub to: NaiveDate,
}

/// 口座全体の日次資産推移
#[utoipa::path(
    get,
    path = "/api/portfolio/history",
    tag = "portfolio",
    params(PortfolioHistoryParams),
    responses(
        (status = 200, body = Vec<PortfolioHistoryPointResponse>),
        (status = 400, description = "期間が不正", body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_portfolio_history(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<PortfolioHistoryParams>,
) -> Result<Json<Vec<PortfolioHistoryPointResponse>>, AppError> {
    let points = state
        .portfolio_history_use_cases
        .history(PortfolioHistoryQuery {
            from: params.from,
            to: params.to,
        })
        .await?;
    Ok(Json(points.into_iter().map(Into::into).collect()))
}
