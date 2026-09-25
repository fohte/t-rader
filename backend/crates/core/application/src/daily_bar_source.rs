use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::bar::Bar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum DailyBarSourceError {
    #[error("instrument not found: {0}")]
    NotFound(String),

    #[error("daily bar source rate limited: {0}")]
    RateLimited(String),

    #[error("daily bar source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait DailyBarSource: Send + Sync {
    async fn fetch_daily_bars(
        &self,
        instrument_id: &str,
        range: &DateRange,
    ) -> Result<Vec<Bar>, DailyBarSourceError>;

    fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)>;
}

pub type SharedDailyBarSource = Arc<dyn DailyBarSource>;
