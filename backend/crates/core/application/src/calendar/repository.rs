use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::calendar_event::CalendarEvent;

use crate::{daily_bar_source::DateRange, persistence::PersistenceError};

#[derive(Debug, thiserror::Error)]
pub enum CalendarEventRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait CalendarEventRepository: Send + Sync {
    async fn upsert(
        &self,
        events: Vec<CalendarEvent>,
    ) -> Result<usize, CalendarEventRepositoryError>;

    async fn delete_missing_future_events(
        &self,
        source: &str,
        date_range: &DateRange,
        today: NaiveDate,
        external_ids: Vec<String>,
    ) -> Result<u64, CalendarEventRepositoryError>;
}

pub type SharedCalendarEventRepository = std::sync::Arc<dyn CalendarEventRepository + Send + Sync>;
