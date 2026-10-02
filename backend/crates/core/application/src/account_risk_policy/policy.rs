use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::error::AccountRiskPolicyDataError;

/// risk_policy JSONB に書き込む際の現行スキーマバージョン。
pub const RISK_POLICY_SCHEMA_VERSION: i32 = 1;

fn current_schema_version() -> i32 {
    RISK_POLICY_SCHEMA_VERSION
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
pub fn validate_ratio(value: Option<Decimal>) -> Result<(), AccountRiskPolicyDataError> {
    let Some(ratio) = value else {
        return Ok(());
    };
    if ratio <= Decimal::ZERO || ratio > Decimal::ONE {
        return Err(AccountRiskPolicyDataError::Validation(
            "ratio must be greater than 0 and less than or equal to 1".into(),
        ));
    }
    Ok(())
}

/// `risk_policy` JSONB カラムの値を型付きデータにパースする。
pub fn parse_risk_policy<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
) -> Result<T, AccountRiskPolicyDataError> {
    serde_json::from_value(value).map_err(AccountRiskPolicyDataError::InvalidData)
}

/// 型付きデータを `risk_policy` JSONB カラムに書き込む値へシリアライズする。
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
    #[case::unset(None, None)]
    #[case::zero(
        Some(Decimal::ZERO),
        Some("ratio must be greater than 0 and less than or equal to 1")
    )]
    #[case::negative(Some(Decimal::from(-1)), Some("ratio must be greater than 0 and less than or equal to 1"))]
    #[case::positive_fraction(Some(Decimal::new(1, 3)), None)]
    #[case::one(Some(Decimal::ONE), None)]
    #[case::above_one(
        Some(Decimal::new(15, 1)),
        Some("ratio must be greater than 0 and less than or equal to 1")
    )]
    fn validate_ratio_checks_the_supported_range(
        #[case] value: Option<Decimal>,
        #[case] expected_error: Option<&str>,
    ) {
        let actual = validate_ratio(value).map_err(|error| error.to_string());
        let expected = expected_error.map_or(Ok(()), |message| Err(message.to_owned()));

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_risk_policy_uses_defaults_and_ignores_unknown_fields() {
        let parsed = parse_risk_policy::<AccountRiskPolicyData>(serde_json::json!({
            "schema_version": 1,
            "max_sector_ratio": "0.15",
            "future_field": "x",
        }))
        .expect("parse policy");
        let missing = parse_risk_policy::<AccountRiskPolicyData>(serde_json::json!({}))
            .expect("parse policy with defaults");

        assert_eq!(
            (parsed, missing),
            (
                AccountRiskPolicyData {
                    schema_version: 1,
                    max_sector_ratio: Some(Decimal::new(15, 2)),
                },
                AccountRiskPolicyData {
                    schema_version: RISK_POLICY_SCHEMA_VERSION,
                    max_sector_ratio: None,
                },
            ),
        );
    }

    #[test]
    fn parse_risk_policy_returns_a_typed_error_for_invalid_data() {
        let actual = parse_risk_policy::<AccountRiskPolicyData>(serde_json::json!({
            "max_sector_ratio": "invalid",
        }));

        assert!(matches!(
            actual,
            Err(AccountRiskPolicyDataError::InvalidData(_))
        ));
    }

    #[test]
    fn serialize_risk_policy_returns_the_stored_json_shape() {
        let data = AccountRiskPolicyData {
            schema_version: RISK_POLICY_SCHEMA_VERSION,
            max_sector_ratio: Some(Decimal::new(3, 1)),
        };

        assert_eq!(
            serialize_risk_policy(&data).expect("serialize policy"),
            serde_json::json!({
                "schema_version": 1,
                "max_sector_ratio": 0.3,
            }),
        );
    }
}
