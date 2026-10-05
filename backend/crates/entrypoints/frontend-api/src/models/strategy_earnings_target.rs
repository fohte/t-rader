use chrono::{DateTime, FixedOffset};
use core_application::strategy_earnings_target::StrategyEarningsTarget;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AddStrategyEarningsTargetRequest {
    pub ref_kind: String,
    pub ref_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveStrategyEarningsTargetQuery {
    pub ref_kind: String,
    pub ref_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyEarningsTargetResponse {
    pub ref_kind: String,
    pub ref_id: String,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
}

impl From<StrategyEarningsTarget> for StrategyEarningsTargetResponse {
    fn from(target: StrategyEarningsTarget) -> Self {
        Self {
            ref_kind: target.ref_kind,
            ref_id: target.ref_id,
            created_at: target.created_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StrategyEarningsTargetChangeResponse {
    pub changed: bool,
}
