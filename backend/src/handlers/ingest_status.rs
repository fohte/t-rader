use axum::Json;
use axum::extract::State;
use chrono::{Duration, Utc};

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::models::IngestStatusResponse;

/// ingest job の実行履歴、データ日、worker queue 状態
#[utoipa::path(
    get,
    path = "/api/ingest-status",
    tag = "ingest_status",
    responses(
        (status = 200, body = IngestStatusResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_ingest_status(
    State(state): State<AppState>,
) -> Result<Json<IngestStatusResponse>, AppError> {
    let today_in_japan = (Utc::now() + Duration::hours(9)).date_naive();
    let status = state.ingest_status_use_case.get(today_in_japan).await?;

    Ok(Json(status.into()))
}
