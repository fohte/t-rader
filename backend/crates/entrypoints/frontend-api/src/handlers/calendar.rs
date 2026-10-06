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
use crate::models::CalendarEventsResponse;

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

fn map_calendar_read_error(error: CalendarEventReadUseCaseError) -> AppError {
    match error {
        error @ CalendarEventReadUseCaseError::InvalidDateRange => {
            AppError::Validation(error.to_string())
        }
        error => AppError::Internal(error.to_string()),
    }
}
