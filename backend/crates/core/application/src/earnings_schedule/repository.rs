use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::earnings_schedule::EarningsSchedule;

use crate::persistence::PersistenceError;

#[derive(Debug, thiserror::Error)]
pub enum EarningsScheduleRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("invalid earnings schedule: {0}")]
    InvalidSchedule(String),
}

#[async_trait]
pub trait EarningsScheduleRepository: Send + Sync {
    async fn latest_published_date(
        &self,
    ) -> Result<Option<NaiveDate>, EarningsScheduleRepositoryError>;

    async fn upsert(
        &self,
        schedules: Vec<EarningsSchedule>,
    ) -> Result<usize, EarningsScheduleRepositoryError>;
}

pub type SharedEarningsScheduleRepository =
    std::sync::Arc<dyn EarningsScheduleRepository + Send + Sync>;
