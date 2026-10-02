//! 口座全体のリスク上限 (`account_risk_policy`)。戦略に紐づかないため 404 判定は無い。

use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use core_application::account_risk_policy::AccountRiskPolicyRepositoryError;

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{
    AccountRiskPolicyData, AccountRiskPolicyResponse, PutAccountRiskPolicyRequest,
    parse_risk_policy, serialize_risk_policy, validate_group_ratios,
};

/// 口座全体の分類軸ごとのグループ集中度上限を取得。
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
        .use_cases
        .account_risk_policies()
        .find_current()
        .await
        .map_err(map_account_risk_policy_error)?;
    let data = match risk_policy {
        Some(risk_policy) => parse_risk_policy(risk_policy)?,
        None => AccountRiskPolicyData {
            schema_version: crate::models::risk_policy::RISK_POLICY_SCHEMA_VERSION,
            max_group_ratios: vec![],
        },
    };
    Ok(Json(data.into()))
}

/// 口座全体の分類軸ごとのグループ集中度上限を更新 (upsert)。空配列で上限を解除する。
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
    validate_group_ratios(&payload.max_group_ratios)?;
    let axes = state.use_cases.group_axes().list().await?;
    let known_axes = axes
        .into_iter()
        .map(|axis| axis.key)
        .collect::<HashSet<_>>();
    if let Some(value) = payload
        .max_group_ratios
        .iter()
        .find(|value| !known_axes.contains(&value.axis))
    {
        return Err(AppError::Validation(format!(
            "unknown axis: {}",
            value.axis
        )));
    }
    let data = AccountRiskPolicyData {
        schema_version: crate::models::risk_policy::RISK_POLICY_SCHEMA_VERSION,
        max_group_ratios: payload.max_group_ratios,
    };
    let value = serialize_risk_policy(&data)?;
    let saved = state
        .use_cases
        .account_risk_policies()
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
    use crate::testing::{create_test_server, insert_test_group};

    #[backend_test_macros::database_test]
    async fn get_returns_null_when_unset(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        let res = server.get("/api/account/risk-policy").await;
        res.assert_status_ok();
        assert_eq!(
            res.json::<serde_json::Value>(),
            serde_json::json!({ "max_group_ratios": [] }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_then_get_round_trips(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        insert_test_group(
            &db,
            "another-sample-axis",
            "another-sample-group",
            "Another sample group",
        )
        .await;
        let server = create_test_server(db).await;

        let expected = serde_json::json!({
            "max_group_ratios": [
                { "axis": "sample-axis", "ratio": 0.3 },
                { "axis": "another-sample-axis", "ratio": 0.2 },
            ]
        });
        let put = server.put("/api/account/risk-policy").json(&expected).await;
        put.assert_status_ok();
        assert_eq!(put.json::<serde_json::Value>(), expected);

        let get = server.get("/api/account/risk-policy").await;
        get.assert_status_ok();
        assert_eq!(get.json::<serde_json::Value>(), expected);
    }

    #[backend_test_macros::database_test]
    async fn put_multiple_times_updates_to_latest_value(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        let server = create_test_server(db).await;

        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            }))
            .await
            .assert_status_ok();
        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            }))
            .await
            .assert_status_ok();

        let get = server.get("/api/account/risk-policy").await;
        assert_eq!(
            get.json::<serde_json::Value>(),
            serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.5 }]
            }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_empty_list_clears_limit(db: gateway_postgres::DatabaseHandle) {
        insert_test_group(&db, "sample-axis", "sample-group", "Sample group").await;
        let server = create_test_server(db).await;

        server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }]
            }))
            .await
            .assert_status_ok();
        let cleared = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({ "max_group_ratios": [] }))
            .await;
        cleared.assert_status_ok();
        assert_eq!(
            cleared.json::<serde_json::Value>(),
            serde_json::json!({ "max_group_ratios": [] }),
        );
    }

    #[backend_test_macros::database_test]
    async fn put_400_for_out_of_range_ratio(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;

        for invalid in [serde_json::json!(0), serde_json::json!(1.5)] {
            let res = server
                .put("/api/account/risk-policy")
                .json(&serde_json::json!({
                    "max_group_ratios": [
                        { "axis": "sample-axis", "ratio": 0.3 },
                        { "axis": "another-sample-axis", "ratio": invalid },
                    ]
                }))
                .await;
            res.assert_status(axum::http::StatusCode::BAD_REQUEST);
        }
    }

    #[backend_test_macros::database_test]
    async fn put_400_for_unknown_axis(db: gateway_postgres::DatabaseHandle) {
        let server = create_test_server(db).await;
        let res = server
            .put("/api/account/risk-policy")
            .json(&serde_json::json!({
                "max_group_ratios": [{ "axis": "unknown-axis", "ratio": 0.3 }]
            }))
            .await;

        res.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }
}
