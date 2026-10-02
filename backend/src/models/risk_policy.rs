use core_application::account_risk_policy::AccountRiskPolicyData;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

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
