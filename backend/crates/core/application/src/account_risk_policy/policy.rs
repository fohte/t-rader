use std::collections::HashSet;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::error::AccountRiskPolicyDataError;

/// risk_policy JSONB に書き込む際の現行スキーマバージョン。
pub const RISK_POLICY_SCHEMA_VERSION: i32 = 2;

fn current_schema_version() -> i32 {
    RISK_POLICY_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GroupRatio {
    /// 分類軸のキー。
    pub axis: String,
    /// 分類軸内の各グループに適用する保有比率の上限。(0, 1] の範囲。
    pub ratio: Decimal,
}

/// account_risk_policy.risk_policy の中身。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountRiskPolicyData {
    #[serde(default = "current_schema_version")]
    pub schema_version: i32,
    /// 分類軸ごとのグループ比率上限。空配列なら上限なし。
    #[serde(default)]
    pub max_group_ratios: Vec<GroupRatio>,
}

/// 分類軸の空値と重複、および比率の範囲を検証する。
pub fn validate_group_ratios(values: &[GroupRatio]) -> Result<(), AccountRiskPolicyDataError> {
    let mut axes = HashSet::with_capacity(values.len());
    for value in values {
        if value.axis.trim().is_empty() {
            return Err(AccountRiskPolicyDataError::Validation(
                "axis must not be empty".into(),
            ));
        }
        if !axes.insert(value.axis.as_str()) {
            return Err(AccountRiskPolicyDataError::Validation(
                "axis must be unique".into(),
            ));
        }
        if value.ratio <= Decimal::ZERO || value.ratio > Decimal::ONE {
            return Err(AccountRiskPolicyDataError::Validation(
                "ratio must be greater than 0 and less than or equal to 1".into(),
            ));
        }
    }
    Ok(())
}

/// risk_policy JSONB カラムの値を型付きデータにパースする。
pub fn parse_risk_policy<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, AccountRiskPolicyDataError> {
    serde_json::from_value(value).map_err(AccountRiskPolicyDataError::InvalidData)
}

/// 型付きデータを risk_policy JSONB カラムに書き込む値へシリアライズする。
pub fn serialize_risk_policy<T: serde::Serialize>(
    data: &T,
) -> Result<serde_json::Value, AccountRiskPolicyDataError> {
    serde_json::to_value(data).map_err(AccountRiskPolicyDataError::InvalidData)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::zero(
        Decimal::ZERO,
        Some("ratio must be greater than 0 and less than or equal to 1")
    )]
    #[case::negative(Decimal::from(-1), Some("ratio must be greater than 0 and less than or equal to 1"))]
    #[case::positive_fraction(Decimal::new(1, 3), None)]
    #[case::one(Decimal::ONE, None)]
    #[case::above_one(
        Decimal::new(15, 1),
        Some("ratio must be greater than 0 and less than or equal to 1")
    )]
    fn validate_group_ratios_checks_the_supported_range(
        #[case] ratio: Decimal,
        #[case] expected_error: Option<&str>,
    ) {
        let values = [GroupRatio {
            axis: "sample-axis".to_string(),
            ratio,
        }];
        let actual = validate_group_ratios(&values).map_err(|error| error.to_string());
        let expected = expected_error.map_or(Ok(()), |message| Err(message.to_owned()));

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::empty_axis(vec![GroupRatio { axis: "  ".to_string(), ratio: Decimal::new(3, 1) }], Some("axis must not be empty"))]
    #[case::duplicate_axis(vec![
        GroupRatio { axis: "sample-axis".to_string(), ratio: Decimal::new(3, 1) },
        GroupRatio { axis: "sample-axis".to_string(), ratio: Decimal::new(2, 1) },
    ], Some("axis must be unique"))]
    #[case::empty_list(vec![], None)]
    fn validate_group_ratios_checks_axis_constraints(
        #[case] values: Vec<GroupRatio>,
        #[case] expected_error: Option<&str>,
    ) {
        let actual = validate_group_ratios(&values).map_err(|error| error.to_string());
        let expected = expected_error.map_or(Ok(()), |message| Err(message.to_owned()));

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_risk_policy_ignores_unknown_fields() {
        let parsed = parse_risk_policy::<AccountRiskPolicyData>(serde_json::json!({
            "schema_version": RISK_POLICY_SCHEMA_VERSION,
            "max_group_ratios": [{ "axis": "sample-axis", "ratio": "0.15" }],
            "future_field": "x",
        }))
        .expect("parse policy");

        assert_eq!(
            parsed,
            AccountRiskPolicyData {
                schema_version: RISK_POLICY_SCHEMA_VERSION,
                max_group_ratios: vec![GroupRatio {
                    axis: "sample-axis".to_string(),
                    ratio: Decimal::new(15, 2),
                }],
            },
        );
    }

    #[test]
    fn parse_risk_policy_uses_defaults_when_fields_are_missing() {
        assert_eq!(
            parse_risk_policy::<AccountRiskPolicyData>(serde_json::json!({}))
                .expect("parse policy with defaults"),
            AccountRiskPolicyData {
                schema_version: RISK_POLICY_SCHEMA_VERSION,
                max_group_ratios: vec![],
            },
        );
    }

    #[test]
    fn parse_risk_policy_returns_a_typed_error_for_invalid_data() {
        let actual = parse_risk_policy::<AccountRiskPolicyData>(serde_json::json!({
            "max_group_ratios": "invalid",
        }))
        .map(|_| ())
        .map_err(|error| match error {
            AccountRiskPolicyDataError::Validation(_) => "validation",
            AccountRiskPolicyDataError::InvalidData(_) => "invalid_data",
        });

        assert_eq!(actual, Err("invalid_data"));
    }

    #[test]
    fn serialize_risk_policy_returns_the_stored_json_shape() {
        let data = AccountRiskPolicyData {
            schema_version: RISK_POLICY_SCHEMA_VERSION,
            max_group_ratios: vec![GroupRatio {
                axis: "sample-axis".to_string(),
                ratio: Decimal::new(3, 1),
            }],
        };

        assert_eq!(
            serialize_risk_policy(&data).expect("serialize policy"),
            serde_json::json!({
                "schema_version": 2,
                "max_group_ratios": [{ "axis": "sample-axis", "ratio": 0.3 }],
            }),
        );
    }
}
