//! J-Quants の契約プラン設定 (`jquants_plan_setting`)。戦略に紐づかないため 404 判定は無い。

use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::data_provider::{DataProvider, DataProviderKind};
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{
    JQuantsPlanSettingData, JQuantsPlanSettingResponse, PutJQuantsPlanSettingRequest,
    parse_plan_setting, serialize_plan_setting,
};
use crate::services::jquants_plan_setting;

/// 現在有効な取得可能範囲。`plan` の手動/自動を問わず `JQuantsClient` に問い合わせるだけでよい
/// (`known_fetchable_range` が手動設定を自動検出より優先する)。
fn effective_range(state: &AppState) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
    state.data_provider.as_ref()?.known_fetchable_range()
}

/// J-Quants の契約プラン設定を取得。未設定なら null (自動検出) を返す
#[utoipa::path(
    get,
    path = "/api/jquants/plan-setting",
    tag = "jquants",
    responses(
        (status = 200, body = JQuantsPlanSettingResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_jquants_plan_setting(
    State(state): State<AppState>,
) -> Result<Json<JQuantsPlanSettingResponse>, AppError> {
    let row = jquants_plan_setting::find_current(&state.db).await?;
    let data = match row {
        Some(row) => parse_plan_setting(row.plan_setting)?,
        None => JQuantsPlanSettingData {
            schema_version: crate::models::jquants_plan::JQUANTS_PLAN_SETTING_SCHEMA_VERSION,
            plan: None,
        },
    };
    let range = effective_range(&state);
    Ok(Json(JQuantsPlanSettingResponse::new(data, range)))
}

/// J-Quants の契約プラン設定を更新 (upsert)。`null` で自動検出に戻す。
/// 保存後、稼働中の `JQuantsClient` があれば即座にインメモリ側も更新する
/// (プロセス再起動なしで反映するため)。
#[utoipa::path(
    put,
    path = "/api/jquants/plan-setting",
    tag = "jquants",
    request_body = PutJQuantsPlanSettingRequest,
    responses(
        (status = 200, body = JQuantsPlanSettingResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_jquants_plan_setting(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<PutJQuantsPlanSettingRequest>,
) -> Result<Json<JQuantsPlanSettingResponse>, AppError> {
    let data = JQuantsPlanSettingData {
        schema_version: crate::models::jquants_plan::JQUANTS_PLAN_SETTING_SCHEMA_VERSION,
        plan: payload.plan,
    };
    let value = serialize_plan_setting(&data)?;
    let saved = jquants_plan_setting::save(&state.db, value).await?;
    let data = parse_plan_setting::<JQuantsPlanSettingData>(saved.plan_setting)?;

    if let Some(provider) = &state.data_provider
        && let DataProviderKind::JQuants(client) = provider.as_ref()
    {
        client.set_manual_plan(data.plan);
    }

    let range = effective_range(&state);
    Ok(Json(JQuantsPlanSettingResponse::new(data, range)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::PgPool;

    use crate::data_provider::DataProvider;
    use crate::data_provider::jquants::JQuantsClient;
    use crate::models::JQuantsPlan;
    use crate::testing::{create_test_server, create_test_server_with_data_provider};

    #[sqlx::test(migrations = false)]
    async fn get_returns_null_when_unset(pool: PgPool) {
        let server = create_test_server(pool).await;

        let res = server.get("/api/jquants/plan-setting").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "plan": null, "effective_range": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;

        let expected = serde_json::json!({ "plan": "standard", "effective_range": null });
        let put = server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": "standard" }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server.get("/api/jquants/plan-setting").await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn put_multiple_times_updates_to_latest_value(pool: PgPool) {
        let server = create_test_server(pool).await;

        server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": "free" }))
            .await
            .assert_status_ok();
        server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": "premium" }))
            .await
            .assert_status_ok();

        let get = server.get("/api/jquants/plan-setting").await;
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({ "plan": "premium", "effective_range": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_null_clears_manual_plan(pool: PgPool) {
        let server = create_test_server(pool).await;

        server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": "standard" }))
            .await
            .assert_status_ok();
        let cleared = server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": null }))
            .await;
        cleared.assert_status_ok();
        assert_eq!(
            cleared.json::<serde_json::Value>(),
            serde_json::json!({ "plan": null, "effective_range": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_422_for_unknown_plan_value(pool: PgPool) {
        let server = create_test_server(pool).await;

        let res = server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": "unlimited" }))
            .await;
        res.assert_status(axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[sqlx::test(migrations = false)]
    async fn put_updates_running_jquants_client_in_memory(pool: PgPool) {
        use crate::data_provider::DataProviderKind;

        let client = JQuantsClient::new("test-key".into()).expect("build client");
        let provider = Arc::new(DataProviderKind::JQuants(client));
        let server = create_test_server_with_data_provider(pool, provider.clone()).await;

        let put = server
            .put("/api/jquants/plan-setting")
            .json(&serde_json::json!({ "plan": "standard" }))
            .await;
        put.assert_status_ok();

        let today = chrono::Utc::now().date_naive();
        let (from, to) = JQuantsPlan::Standard.range(today);
        assert_eq!(
            provider.known_fetchable_range(),
            Some((from, to)),
            "PUT はプロセス再起動なしで稼働中のクライアントへ即座に反映する必要がある"
        );
        assert_eq!(
            put.json::<serde_json::Value>(),
            serde_json::json!({
                "plan": "standard",
                "effective_range": { "from": from, "to": to },
            }),
        );
    }
}
