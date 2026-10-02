//! 口座全体のリスク上限 (`account_risk_policy`)。戦略に紐づかないため 404 判定は無い。

use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use core_application::account_risk_policy::{
    AccountRiskPolicyData, AccountRiskPolicyRepositoryError, GroupRatio as ApplicationGroupRatio,
    RISK_POLICY_SCHEMA_VERSION, parse_risk_policy, serialize_risk_policy, validate_group_ratios,
};

use crate::AppState;
use crate::error::{AppError, ErrorResponse};
use crate::extractors::JsonBody;
use crate::models::{AccountRiskPolicyResponse, PutAccountRiskPolicyRequest};

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
        .account_risk_policy_use_cases
        .find_current()
        .await
        .map_err(map_account_risk_policy_error)?;
    let data = match risk_policy {
        Some(risk_policy) => parse_risk_policy(risk_policy)?,
        None => AccountRiskPolicyData {
            schema_version: RISK_POLICY_SCHEMA_VERSION,
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
    let group_ratios: Vec<ApplicationGroupRatio> = payload
        .max_group_ratios
        .into_iter()
        .map(Into::into)
        .collect();
    validate_group_ratios(&group_ratios)?;
    let axes = state.group_axis_use_cases.list().await?;
    let known_axes = axes
        .into_iter()
        .map(|axis| axis.key)
        .collect::<HashSet<_>>();
    if let Some(value) = group_ratios
        .iter()
        .find(|value| !known_axes.contains(&value.axis))
    {
        return Err(AppError::Validation(format!(
            "unknown axis: {}",
            value.axis
        )));
    }
    let data = AccountRiskPolicyData {
        schema_version: RISK_POLICY_SCHEMA_VERSION,
        max_group_ratios: group_ratios,
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
