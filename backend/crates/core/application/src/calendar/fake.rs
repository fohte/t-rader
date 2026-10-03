use async_trait::async_trait;
use chrono::NaiveDate;

use super::source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError};

#[derive(Debug, Clone)]
pub struct FakeCalendarEventSource {
    source: String,
    result: Result<CalendarEventBatch, CalendarEventSourceError>,
}

impl FakeCalendarEventSource {
    pub fn new(source: impl Into<String>, batch: CalendarEventBatch) -> Self {
        Self {
            source: source.into(),
            result: Ok(batch),
        }
    }

    pub fn failed(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            result: Err(CalendarEventSourceError::Failed(message.into())),
        }
    }
}

#[async_trait]
impl CalendarEventSource for FakeCalendarEventSource {
    fn source(&self) -> &str {
        &self.source
    }

    async fn fetch_calendar_events(
        &self,
        _today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        self.result.clone()
    }
}
