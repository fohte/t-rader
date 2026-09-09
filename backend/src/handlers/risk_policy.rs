//! 口座全体のリスク上限 (`account_risk_policy`)。戦略に紐づかないため 404 判定は無い。

use axum::Json;
use axum::extract::State;
use sea_orm::DbErr;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{
    AccountRiskPolicyData, AccountRiskPolicyResponse, PutAccountRiskPolicyRequest, validate_ratio,
};
use crate::services::account_risk_policy;

fn parse_risk_policy(value: serde_json::Value) -> Result<AccountRiskPolicyData, AppError> {
    serde_json::from_value(value)
        .map_err(|e| AppError::Database(DbErr::Custom(format!("invalid risk_policy: {e}"))))
}

/// 口座全体のセクター集中度上限 (`max_sector_ratio`) を取得。未設定なら null を返す
#[utoipa::path(
    get,
    path = "/api/account/risk-policy",
    tag = "account",
    responses(
        (status = 200, body = AccountRiskPolicyResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn get_account_risk_policy(
    State(state): State<AppState>,
) -> Result<Json<AccountRiskPolicyResponse>, AppError> {
    let row = account_risk_policy::find_current(&state.db).await?;
    let data = match row {
        Some(row) => parse_risk_policy(row.risk_policy)?,
        None => AccountRiskPolicyData {
            schema_version: crate::models::risk_policy::RISK_POLICY_SCHEMA_VERSION,
            max_sector_ratio: None,
        },
    };
    Ok(Json(data.into()))
}

/// 口座全体のセクター集中度上限 (`max_sector_ratio`) を更新 (upsert)。`null` で上限を解除する
#[utoipa::path(
    put,
    path = "/api/account/risk-policy",
    tag = "account",
    request_body = PutAccountRiskPolicyRequest,
    responses(
        (status = 200, body = AccountRiskPolicyResponse),
        (status = 400, body = ErrorResponse),
        (status = 415, description = "Content-Type ヘッダが application/json ではない", body = ErrorResponse),
        (status = 422, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    )
)]
pub async fn put_account_risk_policy(
    State(state): State<AppState>,
    JsonBody(payload): JsonBody<PutAccountRiskPolicyRequest>,
) -> Result<Json<AccountRiskPolicyResponse>, AppError> {
    validate_ratio(payload.max_sector_ratio)?;
    let data = AccountRiskPolicyData {
        schema_version: crate::models::risk_policy::RISK_POLICY_SCHEMA_VERSION,
        max_sector_ratio: payload.max_sector_ratio,
    };
    let value = serde_json::to_value(&data)
        .map_err(|e| AppError::Database(DbErr::Custom(format!("invalid risk_policy: {e}"))))?;
    let saved = account_risk_policy::save(&state.db, value).await?;
    let data = parse_risk_policy(saved.risk_policy)?;
    Ok(Json(data.into()))
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::testing::create_test_server;

    #[sqlx::test(migrations = false)]
    async fn get_returns_null_when_unset(pool: PgPool) {
        let server = create_test_server(pool).await;

        let res = server.get("/api/account/risk-policy").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "max_sector_ratio": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_then_get_round_trips(pool: PgPool) {
        let server = create_test_server(pool).await;

        let expected = serde_json::json!({ "max_sector_ratio": 0.3 });
        let put = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_sector_ratio": 0.3 }))
            .await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server.get("/api/account/risk-policy").await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
    }

    #[sqlx::test(migrations = false)]
    async fn put_multiple_times_updates_to_latest_value(pool: PgPool) {
        let server = create_test_server(pool).await;

        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_sector_ratio": 0.3 }))
            .await
            .assert_status_ok();
        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_sector_ratio": 0.5 }))
            .await
            .assert_status_ok();

        let get = server.get("/api/account/risk-policy").await;
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({ "max_sector_ratio": 0.5 }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_null_clears_limit(pool: PgPool) {
        let server = create_test_server(pool).await;

        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_sector_ratio": 0.3 }))
            .await
            .assert_status_ok();
        let cleared = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_sector_ratio": null }))
            .await;
        cleared.assert_status_ok();
        assert_eq!(
            cleared.json::<serde_json::Value>(),
            serde_json::json!({ "max_sector_ratio": null }),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn put_400_for_out_of_range_ratio(pool: PgPool) {
        let server = create_test_server(pool).await;

        for invalid in [serde_json::json!(0), serde_json::json!(1.5)] {
            let res = server
                .put("/api/account/risk-policy")
                .json(&serde_json::json!({ "max_sector_ratio": invalid }))
                .await;
            res.assert_status(axum::http::StatusCode::BAD_REQUEST);
        }
    }
}
