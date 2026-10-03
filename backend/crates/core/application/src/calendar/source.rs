use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::calendar_event::CalendarEvent;

use crate::daily_bar_source::DateRange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventBatch {
    pub date_range: DateRange,
    pub events: Vec<CalendarEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CalendarEventSourceError {
    #[error("calendar event source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait CalendarEventSource: Send + Sync {
    fn source(&self) -> &str;

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError>;
}

pub type SharedCalendarEventSource = Arc<dyn CalendarEventSource>;
