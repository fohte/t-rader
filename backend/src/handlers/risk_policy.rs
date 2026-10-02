//! 口座全体のリスク上限 (`account_risk_policy`)。戦略に紐づかないため 404 判定は無い。

use axum::Json;
use axum::extract::State;
use core_application::account_risk_policy::AccountRiskPolicyRepositoryError;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{
    AccountRiskPolicyData, AccountRiskPolicyResponse, PutAccountRiskPolicyRequest,
    parse_risk_policy, serialize_risk_policy, validate_ratio,
};

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
    let risk_policy = state
        .account_risk_policy_use_cases
        .find_current()
        .await
        .map_err(map_account_risk_policy_error)?;
    let data = match risk_policy {
        Some(risk_policy) => parse_risk_policy(risk_policy)?,
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
    let value = serialize_risk_policy(&data)?;
    let saved = state
        .account_risk_policy_use_cases
        .save(value)
        .await
        .map_err(map_account_risk_policy_error)?;
    let data = parse_risk_policy::<AccountRiskPolicyData>(saved)?;
    Ok(Json(data.into()))
}

fn map_account_risk_policy_error(error: AccountRiskPolicyRepositoryError) -> AppError {
    match error {
        AccountRiskPolicyRepositoryError::Database(error) => error.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::create_test_server;

    #[backend_test_macros::database_test]
    async fn get_returns_null_when_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let res = server.get("/api/account/risk-policy").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "max_sector_ratio": null }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_round_trips(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

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

    #[backend_test_macros::database_test]
    async fn put_multiple_times_updates_to_latest_value(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

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

    #[backend_test_macros::database_test]
    async fn put_null_clears_limit(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

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

    #[backend_test_macros::database_test]
    async fn put_400_for_out_of_range_ratio(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        for invalid in [serde_json::json!(0), serde_json::json!(1.5)] {
            let res = server
                .put("/api/account/risk-policy")
                .json(&serde_json::json!({ "max_sector_ratio": invalid }))
                .await;
            res.assert_status(axum::http::StatusCode::BAD_REQUEST);
        }
    }
}
