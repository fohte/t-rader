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
    // poll task が panic 等で silent に止まった場合 `stale_since` が更新されない。
    // 個々の tick の `fetched_at` も 24h 越えで N/A 扱いにし、防御層を二重化する。
    let too_stale = snapshot
        .stale_since
        .map(|s| now.signed_duration_since(s) > STALE_CUTOFF)
        .unwrap_or(false)
        || snapshot
            .ticks
            .first()
            .map(|t| now.signed_duration_since(t.fetched_at) > STALE_CUTOFF)
            .unwrap_or(false);
    // 起動直後の初回 poll 完了前 (ticks=[], stale_since=None) と、初回 poll 失敗後
    // (ticks=[], stale_since=Some) の両方を「まだ取得結果がない」状態として N/A 扱いにする。
    // frontend は `ticks: null` を loading / N/A 分岐に倒す。
    let no_data_yet = snapshot.ticks.is_empty();

    let ticks = if too_stale || no_data_yet {
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
    use serde_json::{Value, json};

    use super::*;
    use crate::AppState;
    use crate::create_router;
    use crate::data_provider::macro_data::MacroCache;

    fn dec(s: &str) -> Decimal {
        Decimal::from_str(s).expect("valid decimal")
    }

    /// テストデータの `fetched_at` には `Utc::now()` を入れる。固定の未来日付を
    /// 使うとその日が来たときに handler の 24h cutoff で挙動が変わり time-bomb
    /// テストになるため。アサーションでは [`normalize_fetched_at`] で固定値に正規化する。
    fn sample_tick() -> MacroTick {
        MacroTick {
            symbol: "日経225".into(),
            value: "100.00".into(),
            pct: dec("0.50"),
            fetched_at: Utc::now(),
        }
    }

    const NORMALIZED_FETCHED_AT: &str = "1970-01-01T00:00:00Z";

    /// レスポンス JSON の `ticks[].fetched_at` を固定値に置き換える
    fn normalize_fetched_at(mut json: Value) -> Value {
        if let Some(ticks) = json.get_mut("ticks").and_then(|t| t.as_array_mut()) {
            for tick in ticks {
                if let Some(fetched_at) = tick.get_mut("fetched_at") {
                    *fetched_at = json!(NORMALIZED_FETCHED_AT);
                }
            }
        }
        json
    }

    fn ymd_hms(year: i32, mon: u32, day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, mon, day, h, m, s)
            .single()
            .expect("valid time")
    }

    /// cache 状態のセットアップパターン
    enum CacheState {
        Fresh,
        StaleOver24h,
        FailedBeforeFirstSuccess,
        /// poll task が silent に止まり stale_since 更新が来ない状態を模す
        /// (record_success の tick だけが古いまま残る)
        PollTaskDiedSilently,
    }

    async fn build_cache(state: CacheState) -> Arc<MacroCache> {
        let cache = Arc::new(MacroCache::new());
        match state {
            CacheState::Fresh => {
                cache.record_success(vec![sample_tick()]).await;
            }
            CacheState::StaleOver24h => {
                cache.record_success(vec![sample_tick()]).await;
                cache.record_failure(ymd_hms(2020, 1, 1, 0, 0, 0)).await;
            }
            CacheState::FailedBeforeFirstSuccess => {
                cache.record_failure(ymd_hms(2026, 6, 25, 5, 30, 0)).await;
            }
            CacheState::PollTaskDiedSilently => {
                cache
                    .record_success(vec![MacroTick {
                        fetched_at: ymd_hms(2020, 1, 1, 0, 0, 0),
                        ..sample_tick()
                    }])
                    .await;
            }
        }
        cache
    }

    fn build_router(macro_cache: Option<Arc<MacroCache>>) -> Router {
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState {
            db,
            data_provider: None,
            agent_task_client: AppState::disabled_agent_task_client(),
            agent_task_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
            agent_webhook_token: std::sync::Arc::from("test-token"),
            kata_executor: None,
            macro_cache,
            llm_gateway_client: None,
        };
        create_router(state)
    }

    #[rstest]
    #[case::fresh(
        CacheState::Fresh,
        json!({
            "ticks": [{
                "symbol": "日経225",
                "value": "100.00",
                "pct": 0.5,
                "fetched_at": NORMALIZED_FETCHED_AT,
            }],
            "stale_since": null,
        }),
    )]
    #[case::stale_over_24h(
        CacheState::StaleOver24h,
        json!({
            "ticks": null,
            "stale_since": "2020-01-01T00:00:00Z",
        }),
    )]
    #[case::failed_before_first_success(
        CacheState::FailedBeforeFirstSuccess,
        json!({
            "ticks": null,
            "stale_since": "2026-06-25T05:30:00Z",
        }),
    )]
    #[case::poll_task_died_silently(
        CacheState::PollTaskDiedSilently,
        json!({
            "ticks": null,
            "stale_since": null,
        }),
    )]
    #[tokio::test]
    async fn get_macro_ticks_response(#[case] state: CacheState, #[case] expected: Value) {
        let cache = build_cache(state).await;
        let server = TestServer::new(build_router(Some(cache))).expect("server");

        let response = server.get("/api/macro/ticks").await;
        response.assert_status_ok();
        assert_eq!(normalize_fetched_at(response.json::<Value>()), expected);
    }

    #[rstest]
    #[tokio::test]
    async fn returns_503_when_cache_not_configured() {
        let server = TestServer::new(build_router(None)).expect("server");
        let response = server.get("/api/macro/ticks").await;

        response.assert_status(StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.json::<Value>(),
            json!({ "error": "macro provider is not configured" }),
        );
    }
}
