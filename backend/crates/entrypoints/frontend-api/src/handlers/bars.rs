use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use core_application::bars::BarsQuery;
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
    /// 取得開始日 (YYYY-MM-DD, inclusive)
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD, inclusive)
    pub to: Option<NaiveDate>,
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

    // Bar.timeframe の OpenAPI スキーマは DTO の String 型から導出されるため許容値を含まない。
    // 実際の許容値はこの配列と bars テーブルの CHECK 制約が正とする。
    let valid_timeframes = ["1d"];
    if !valid_timeframes.contains(&params.timeframe.as_str()) {
        return Err(AppError::Validation(format!(
            "invalid timeframe: {}. valid values: {:?}",
            params.timeframe, valid_timeframes
        )));
    }

    // NaiveDate -> DateTime<FixedOffset> に変換
    // from: その日の 00:00:00 UTC
    // to: その日の 23:59:59 UTC (inclusive)
    let from = params
        .from
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().fixed_offset());

    let to = params
        .to
        .and_then(|d| d.and_hms_opt(23, 59, 59))
        .map(|dt| dt.and_utc().fixed_offset());

    let query = BarsQuery {
        instrument_id: params.instrument_id,
        timeframe: params.timeframe,
        from,
        to,
    };

    let bars = state.bars_use_cases.find_bars(query).await?;

    Ok(Json(bars.into_iter().map(BarResponse::from).collect()))
}
