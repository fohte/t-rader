use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use core_application::calendar::read_use_cases::CalendarEventReadUseCaseError;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::handlers::strategies::strategy_scope_or_404;
use crate::models::{CalendarEventsResponse, CalendarOtherEarningsResponse};

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CalendarEventsQuery {
    /// 取得開始日。省略時は今週の月曜日 (JST)
    pub from: Option<NaiveDate>,
    /// 取得終了日。省略時は今週の日曜日 (JST)
    pub to: Option<NaiveDate>,
    /// 決算対象を絞り込む戦略 ID。省略時は決算を日別件数にまとめる
    pub strategy_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CalendarOtherEarningsQuery {
    /// 決算日
    pub event_date: NaiveDate,
    /// 上場市場の国・地域
    pub country: String,
    /// 決算対象を絞り込む戦略 ID
    pub strategy_id: Option<Uuid>,
}

/// 指定期間のイベントを一覧する
#[utoipa::path(
    get,
    path = "/api/calendar/events",
    tag = "calendar",
    params(CalendarEventsQuery),
    responses(
        (status = 200, description = "イベント一覧", body = CalendarEventsResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_calendar_events(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<CalendarEventsQuery>,
) -> Result<Json<CalendarEventsResponse>, AppError> {
    let strategy_scope = match params.strategy_id {
        Some(id) => Some(strategy_scope_or_404(&state, id).await?),
        None => None,
    };
    let result = state
        .calendar_event_read_use_cases
        .list_events(params.from, params.to, strategy_scope)
        .await
        .map_err(map_calendar_read_error)?;

    Ok(Json(result.into()))
}

/// 指定した国・日の集約対象決算を一覧する
#[utoipa::path(
    get,
    path = "/api/calendar/other-earnings",
    tag = "calendar",
    params(CalendarOtherEarningsQuery),
    responses(
        (status = 200, description = "集約対象の決算一覧", body = CalendarOtherEarningsResponse),
        (status = 400, body = ErrorResponse),
        (status = 404, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn list_other_earnings(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<CalendarOtherEarningsQuery>,
) -> Result<Json<CalendarOtherEarningsResponse>, AppError> {
    let strategy_scope = match params.strategy_id {
        Some(id) => Some(strategy_scope_or_404(&state, id).await?),
        None => None,
    };
    let events = state
        .calendar_event_read_use_cases
        .list_other_earnings(params.event_date, &params.country, strategy_scope)
        .await
        .map_err(map_calendar_read_error)?;

    Ok(Json(events.into()))
}

fn map_calendar_read_error(error: CalendarEventReadUseCaseError) -> AppError {
    match error {
        error @ CalendarEventReadUseCaseError::InvalidDateRange => {
            AppError::Validation(error.to_string())
        }
        error => AppError::Internal(error.to_string()),
    }
}
