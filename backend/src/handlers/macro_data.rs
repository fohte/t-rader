use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::data_provider::macro_data::MacroTick;
use crate::error::{AppError, ErrorResponse};

/// `stale_since` がこの期間より古い場合は値を返さない (frontend で N/A 表示)
const STALE_CUTOFF: Duration = Duration::hours(24);

/// マクロ指標ティック取得レスポンス
#[derive(Debug, Serialize, ToSchema)]
pub struct MacroTicksResponse {
    /// 直近の取得値。一度も成功していない、または 24h 以上失敗が続いている場合は `null`
    pub ticks: Option<Vec<MacroTick>>,
    /// 直近の取得失敗が継続している場合、その失敗の開始時刻
    pub stale_since: Option<DateTime<Utc>>,
}

/// マクロ指標の現在値を取得する
#[utoipa::path(
    get,
    path = "/api/macro/ticks",
    tag = "macro",
    responses(
        (status = 200, description = "現在値", body = MacroTicksResponse),
        (status = 503, description = "macro provider 未設定", body = ErrorResponse),
    )
)]
pub async fn get_macro_ticks(
    State(state): State<AppState>,
) -> Result<Json<MacroTicksResponse>, AppError> {
    let cache = state
        .macro_cache
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("macro provider is not configured".into()))?;

    let snapshot = cache.snapshot().await;
    let now = Utc::now();
    let too_stale = snapshot
        .stale_since
        .map(|s| now.signed_duration_since(s) > STALE_CUTOFF)
        .unwrap_or(false);
    // 一度も成功していない (起動直後の poll 失敗) ケースも N/A 扱いに揃える
    let never_succeeded = snapshot.ticks.is_empty() && snapshot.stale_since.is_some();

    let ticks = if too_stale || never_succeeded {
        None
    } else {
        Some(snapshot.ticks)
    };

    Ok(Json(MacroTicksResponse {
        ticks,
        stale_since: snapshot.stale_since,
    }))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use axum::Router;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use chrono::TimeZone;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::{DatabaseBackend, MockDatabase};
    use serde_json::json;

    use super::*;
    use crate::AppState;
    use crate::create_router;
    use crate::data_provider::macro_data::MacroCache;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal")
    }

    fn sample_tick() -> MacroTick {
        MacroTick {
            symbol: "日経225".into(),
            value: "100.00".into(),
            pct: dec("0.50"),
            fetched_at: Utc
                .with_ymd_and_hms(2026, 6, 25, 6, 0, 0)
                .single()
                .expect("ok"),
        }
    }

    fn build_router(macro_cache: Option<Arc<MacroCache>>) -> Router {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState {
            db,
            data_provider: None,
            kubeopencode: AppState::disabled_kubeopencode(),
            kata_executor: None,
            macro_cache,
        };
        create_router(state)
    }

    #[rstest]
    #[tokio::test]
    async fn returns_ticks_when_fresh() {
        let cache = Arc::new(MacroCache::new());
        cache.record_success(vec![sample_tick()]).await;
        let server = TestServer::new(build_router(Some(cache))).expect("server");

        let response = server.get("/api/macro/ticks").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            json!({
                "ticks": [{
                    "symbol": "日経225",
                    "value": "100.00",
                    "pct": 0.5,
                    "fetched_at": "2026-06-25T06:00:00Z",
                }],
                "stale_since": null,
            }),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn returns_null_ticks_when_stale_over_24h() {
        let cache = Arc::new(MacroCache::new());
        cache.record_success(vec![sample_tick()]).await;
        let stale_at = Utc
            .with_ymd_and_hms(2020, 1, 1, 0, 0, 0)
            .single()
            .expect("ok");
        cache.record_failure(stale_at).await;
        let server = TestServer::new(build_router(Some(cache))).expect("server");

        let response = server.get("/api/macro/ticks").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            json!({
                "ticks": null,
                "stale_since": "2020-01-01T00:00:00Z",
            }),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn returns_null_ticks_when_never_succeeded() {
        let cache = Arc::new(MacroCache::new());
        let failed_at = Utc
            .with_ymd_and_hms(2026, 6, 25, 5, 30, 0)
            .single()
            .expect("ok");
        cache.record_failure(failed_at).await;
        let server = TestServer::new(build_router(Some(cache))).expect("server");

        let response = server.get("/api/macro/ticks").await;
        response.assert_status_ok();
        assert_eq!(
            response.json::<serde_json::Value>(),
            json!({
                "ticks": null,
                "stale_since": "2026-06-25T05:30:00Z",
            }),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn returns_503_when_cache_not_configured() {
        let server = TestServer::new(build_router(None)).expect("server");
        let response = server.get("/api/macro/ticks").await;
        response.assert_status(StatusCode::SERVICE_UNAVAILABLE);
    }
}
