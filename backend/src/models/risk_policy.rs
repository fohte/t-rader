use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// risk_policy JSONB に書き込む際の現行スキーマバージョン。
pub const RISK_POLICY_SCHEMA_VERSION: i32 = 1;

fn current_schema_version() -> i32 {
    RISK_POLICY_SCHEMA_VERSION
}

/// `strategy.risk_policy` の中身。分子は銘柄の保有時価、分母はその戦略の投資可能額
/// (`strategy_investable_amount` の現在値)。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StrategyRiskPolicyData {
    #[serde(default = "current_schema_version")]
    pub schema_version: i32,
    /// (0, 1] の範囲。未設定 (上限なし) なら `None`
    #[serde(default)]
    pub max_position_ratio: Option<Decimal>,
}

/// `account_risk_policy.risk_policy` の中身。分子はそのセクターに属する保有銘柄の時価合計
/// (口座全体、全戦略横断)、分母は口座全体の保有銘柄時価合計。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountRiskPolicyData {
    #[serde(default = "current_schema_version")]
    pub schema_version: i32,
    /// (0, 1] の範囲。未設定 (上限なし) なら `None`
    #[serde(default)]
    pub max_sector_ratio: Option<Decimal>,
}

/// 比率の範囲を検証する。`None` (未設定) は許可する。
pub fn validate_ratio(value: Option<Decimal>) -> Result<(), crate::error::AppError> {
    let Some(ratio) = value else {
        return Ok(());
    };
    if ratio <= Decimal::ZERO || ratio > Decimal::ONE {
        return Err(crate::error::AppError::Validation(
            "ratio must be greater than 0 and less than or equal to 1".into(),
        ));
    }
    Ok(())
}

/// `risk_policy` JSONB カラムの値を型付きデータにパースする。
pub fn parse_risk_policy<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, crate::error::AppError> {
    serde_json::from_value(value).map_err(|e| {
        crate::error::AppError::Database(sea_orm::DbErr::Custom(format!(
            "invalid risk_policy: {e}"
        )))
    })
}

/// 型付きデータを `risk_policy` JSONB カラムに書き込む値へシリアライズする。
pub fn serialize_risk_policy<T: serde::Serialize>(
    data: &T,
) -> Result<serde_json::Value, crate::error::AppError> {
    serde_json::to_value(data).map_err(|e| {
        crate::error::AppError::Database(sea_orm::DbErr::Custom(format!(
            "invalid risk_policy: {e}"
        )))
    })
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutStrategyRiskPolicyRequest {
    /// 銘柄の保有時価 / 戦略の投資可能額 の上限比率。(0, 1] の範囲。`null` で上限を解除する
    pub max_position_ratio: Option<Decimal>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyRiskPolicyResponse {
    pub max_position_ratio: Option<Decimal>,
}

impl From<StrategyRiskPolicyData> for StrategyRiskPolicyResponse {
    fn from(data: StrategyRiskPolicyData) -> Self {
        Self {
            max_position_ratio: data.max_position_ratio,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutAccountRiskPolicyRequest {
    /// セクターに属する保有銘柄の時価合計 (口座全体) / 口座全体の保有銘柄時価合計 の上限比率。
    /// (0, 1] の範囲。`null` で上限を解除する
    pub max_sector_ratio: Option<Decimal>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountRiskPolicyResponse {
    pub max_sector_ratio: Option<Decimal>,
}

impl From<AccountRiskPolicyData> for AccountRiskPolicyResponse {
    fn from(data: AccountRiskPolicyData) -> Self {
        Self {
            max_sector_ratio: data.max_sector_ratio,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::none(None, true)]
    #[case::zero(Some(Decimal::ZERO), false)]
    #[case::negative(Some(Decimal::from(-1)), false)]
    #[case::lower_bound_exclusive(Some(Decimal::new(1, 3)), true)]
    #[case::one_inclusive(Some(Decimal::ONE), true)]
    #[case::above_one(Some(Decimal::new(15, 1)), false)]
    fn test_validate_ratio(#[case] value: Option<Decimal>, #[case] expect_ok: bool) {
        assert_eq!(validate_ratio(value).is_ok(), expect_ok);
    }

    #[test]
    fn test_strategy_risk_policy_data_defaults_unknown_and_missing_fields() {
        let value: StrategyRiskPolicyData = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "max_position_ratio": "0.15",
            "future_field": "x",
        }))
        .expect("parse");
        assert_eq!(
            value,
            StrategyRiskPolicyData {
                schema_version: 1,
                max_position_ratio: Some(Decimal::new(15, 2)),
            }
        );

        let missing: StrategyRiskPolicyData =
            serde_json::from_value(serde_json::json!({})).expect("parse with defaults");
        assert_eq!(
            missing,
            StrategyRiskPolicyData {
                schema_version: RISK_POLICY_SCHEMA_VERSION,
                max_position_ratio: None,
            }
        );
    }
}
