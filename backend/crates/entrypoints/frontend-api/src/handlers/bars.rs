use axum::Json;
use axum::extract::State;
use chrono::{DateTime, FixedOffset, NaiveDate};
use core_application::bars::BarsQuery;
use core_domain::bar::Timeframe;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::FrontendApiState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonQuery;
use crate::models::BarResponse;

/// バーデータ取得のクエリパラメータ
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BarsQueryParams {
    /// 銘柄コード (必須)
    pub instrument_id: String,
    /// 時間足 (デフォルト: "1d")
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    /// 取得開始日時。日足は YYYY-MM-DD、日足以外は RFC 3339 datetime (inclusive)
    pub from: Option<String>,
    /// 取得終了日時。日足は YYYY-MM-DD、日足以外は RFC 3339 datetime (inclusive)
    pub to: Option<String>,
}

fn default_timeframe() -> String {
    "1d".to_string()
}

/// バーデータを取得する
#[utoipa::path(
    get,
    path = "/api/bars",
    tag = "bars",
    params(BarsQueryParams),
    responses(
        (status = 200, description = "バーデータ一覧", body = Vec<BarResponse>),
        (status = 400, description = "バリデーションエラー", body = ErrorResponse),
        (status = 500, description = "内部サーバーエラー", body = ErrorResponse),
    )
)]
pub async fn list_bars(
    State(state): State<FrontendApiState>,
    JsonQuery(params): JsonQuery<BarsQueryParams>,
) -> Result<Json<Vec<BarResponse>>, AppError> {
    if params.instrument_id.trim().is_empty() {
        return Err(AppError::Validation(
            "instrument_id must not be empty".to_string(),
        ));
    }

    let timeframe = params.timeframe.parse::<Timeframe>().map_err(|_| {
        let valid_timeframes = Timeframe::ALL.map(|timeframe| timeframe.to_string());
        AppError::Validation(format!(
            "invalid timeframe: {}. valid values: {:?}",
            params.timeframe, valid_timeframes
        ))
    })?;

    let (from, to) = parse_range(timeframe, params.from.as_deref(), params.to.as_deref())?;

    let query = BarsQuery {
        instrument_id: params.instrument_id,
        timeframe: timeframe.to_string(),
        from,
        to,
    };

    let bars = state.bars_use_cases.find_bars(query).await?;

    Ok(Json(bars.into_iter().map(BarResponse::from).collect()))
}

type DateTimeRange = (Option<DateTime<FixedOffset>>, Option<DateTime<FixedOffset>>);

fn parse_range(
    timeframe: Timeframe,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<DateTimeRange, AppError> {
    let parse_boundary = |value: &str, parameter: &str, is_end_of_day: bool| {
        if timeframe == Timeframe::Daily {
            let date = value.parse::<NaiveDate>().map_err(|_| {
                AppError::Validation(format!("{parameter} must be YYYY-MM-DD for 1d"))
            })?;
            let time = if is_end_of_day {
                (23, 59, 59)
            } else {
                (0, 0, 0)
            };
            return date
                .and_hms_opt(time.0, time.1, time.2)
                .map(|datetime| datetime.and_utc().fixed_offset())
                .ok_or_else(|| AppError::Validation(format!("invalid {parameter} date")));
        }

        DateTime::parse_from_rfc3339(value).map_err(|_| {
            AppError::Validation(format!(
                "{parameter} must be an RFC 3339 datetime for intraday timeframes"
            ))
        })
    };

    let from = from
        .map(|value| parse_boundary(value, "from", false))
        .transpose()?;
    let to = to
        .map(|value| parse_boundary(value, "to", true))
        .transpose()?;

    if matches!((&from, &to), (Some(from), Some(to)) if from > to) {
        return Err(AppError::Validation(
            "from must be earlier than or equal to to".to_string(),
        ));
    }

    Ok((from, to))
}
