use chrono::{DateTime, FixedOffset};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyEarningsTarget {
    pub ref_kind: String,
    pub ref_id: String,
    pub created_at: DateTime<FixedOffset>,
}
