use std::collections::HashSet;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// risk_policy JSONB に書き込む際の現行スキーマバージョン。
pub const RISK_POLICY_SCHEMA_VERSION: i32 = 2;

fn current_schema_version() -> i32 {
    RISK_POLICY_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct GroupRatio {
    /// 分類軸のキー。
    pub axis: String,
    /// 分類軸内の各グループに適用する保有比率の上限。(0, 1] の範囲。
    pub ratio: Decimal,
}

/// `account_risk_policy.risk_policy` の中身。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountRiskPolicyData {
    #[serde(default = "current_schema_version")]
    pub schema_version: i32,
    /// 分類軸ごとのグループ比率上限。空配列なら上限なし。
    #[serde(default)]
    pub max_group_ratios: Vec<GroupRatio>,
}

/// 分類軸の空値と重複、および比率の範囲を検証する。
pub fn validate_group_ratios(values: &[GroupRatio]) -> Result<(), crate::error::AppError> {
    let mut axes = HashSet::with_capacity(values.len());
    for value in values {
        if value.axis.trim().is_empty() {
            return Err(crate::error::AppError::Validation(
                "axis must not be empty".into(),
            ));
        }
        if !axes.insert(value.axis.as_str()) {
            return Err(crate::error::AppError::Validation(
                "axis must be unique".into(),
            ));
        }
        if value.ratio <= Decimal::ZERO || value.ratio > Decimal::ONE {
            return Err(crate::error::AppError::Validation(
                "ratio must be greater than 0 and less than or equal to 1".into(),
            ));
        }
    }
    Ok(())
}

/// `risk_policy` JSONB カラムの値を型付きデータにパースする。
pub fn parse_risk_policy<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, crate::error::AppError> {
    serde_json::from_value(value)
        .map_err(|e| crate::error::AppError::Internal(format!("invalid risk_policy: {e}")))
}

/// 型付きデータを `risk_policy` JSONB カラムに書き込む値へシリアライズする。
pub fn serialize_risk_policy<T: serde::Serialize>(
    data: &T,
) -> Result<serde_json::Value, crate::error::AppError> {
    serde_json::to_value(data)
        .map_err(|e| crate::error::AppError::Internal(format!("invalid risk_policy: {e}")))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutAccountRiskPolicyRequest {
    /// 分類軸ごとのグループ比率上限。空配列なら上限なし。
    pub max_group_ratios: Vec<GroupRatio>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountRiskPolicyResponse {
    /// 分類軸ごとのグループ比率上限。空配列なら上限なし。
    pub max_group_ratios: Vec<GroupRatio>,
}

impl From<AccountRiskPolicyData> for AccountRiskPolicyResponse {
    fn from(data: AccountRiskPolicyData) -> Self {
        Self {
            max_group_ratios: data.max_group_ratios,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::zero(Decimal::ZERO, false)]
    #[case::negative(Decimal::from(-1), false)]
    #[case::lower_bound_exclusive(Decimal::new(1, 3), true)]
    #[case::one_inclusive(Decimal::ONE, true)]
    #[case::above_one(Decimal::new(15, 1), false)]
    fn test_validate_group_ratios(#[case] ratio: Decimal, #[case] expect_ok: bool) {
        let values = [GroupRatio {
            axis: "sample-axis".to_string(),
            ratio,
        }];
        assert_eq!(validate_group_ratios(&values).is_ok(), expect_ok);
    }

    #[test]
    fn test_validate_group_ratios_accepts_empty_list() {
        assert!(validate_group_ratios(&[]).is_ok());
    }

    #[test]
    fn test_validate_group_ratios_rejects_empty_axis() {
        let values = [GroupRatio {
            axis: "  ".to_string(),
            ratio: Decimal::new(3, 1),
        }];
        assert!(validate_group_ratios(&values).is_err());
    }

    #[test]
    fn test_validate_group_ratios_rejects_duplicate_axes() {
        let values = [
            GroupRatio {
                axis: "sample-axis".to_string(),
                ratio: Decimal::new(3, 1),
            },
            GroupRatio {
                axis: "sample-axis".to_string(),
                ratio: Decimal::new(2, 1),
            },
        ];
        assert!(validate_group_ratios(&values).is_err());
    }

    #[test]
    fn test_account_risk_policy_data_defaults_unknown_and_missing_fields() {
        let value: AccountRiskPolicyData = serde_json::from_value(serde_json::json!({
            "schema_version": RISK_POLICY_SCHEMA_VERSION,
            "max_group_ratios": [{"axis": "sample-axis", "ratio": "0.15"}],
            "future_field": "x",
        }))
        .expect("parse");
        assert_eq!(
            value,
            AccountRiskPolicyData {
                schema_version: RISK_POLICY_SCHEMA_VERSION,
                max_group_ratios: vec![GroupRatio {
                    axis: "sample-axis".to_string(),
                    ratio: Decimal::new(15, 2),
                }],
            }
        );

        let missing: AccountRiskPolicyData =
            serde_json::from_value(serde_json::json!({})).expect("parse with defaults");
        assert_eq!(
            missing,
            AccountRiskPolicyData {
                schema_version: RISK_POLICY_SCHEMA_VERSION,
                max_group_ratios: vec![],
            }
        );
    }
}
