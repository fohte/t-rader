use core_application::account_risk_policy::{
    AccountRiskPolicyData, GroupRatio as ApplicationGroupRatio,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
pub struct GroupRatio {
    /// 分類軸のキー。
    pub axis: String,
    /// 分類軸内の各グループに適用する保有比率の上限。(0, 1] の範囲。
    pub ratio: Decimal,
}

impl From<ApplicationGroupRatio> for GroupRatio {
    fn from(value: ApplicationGroupRatio) -> Self {
        Self {
            axis: value.axis,
            ratio: value.ratio,
        }
    }
}

impl From<GroupRatio> for ApplicationGroupRatio {
    fn from(value: GroupRatio) -> Self {
        Self {
            axis: value.axis,
            ratio: value.ratio,
        }
    }
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
            max_group_ratios: data.max_group_ratios.into_iter().map(Into::into).collect(),
        }
    }
}
