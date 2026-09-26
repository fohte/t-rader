use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::earnings_schedule::EarningsSchedule;

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum EarningsScheduleSourceError {
    #[error("earnings schedule source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait EarningsScheduleSource: Send + Sync {
    /// 指定した公表日の決算発表予定を取得する。
    async fn fetch_earnings_schedules_by_date(
        &self,
        published_date: NaiveDate,
    ) -> Result<Vec<EarningsSchedule>, EarningsScheduleSourceError>;

    /// `today` 時点で取得できる日付の範囲。取得できない間は `None`。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedEarningsScheduleSource = Arc<dyn EarningsScheduleSource>;
