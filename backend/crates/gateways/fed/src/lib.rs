//! FRB の FOMC 会合カレンダーを取得する gateway。

mod macro_references;
mod official_http;
mod parser;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::source::{
    CalendarEventBatch, CalendarEventSource, CalendarEventSourceError,
};
use official_http::OfficialHttpClient;

pub use macro_references::FedMacroReferenceSource;

const CALENDAR_URL: &str = "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm";
const SOURCE: &str = "fed";

pub struct FedClient {
    http: OfficialHttpClient,
}

impl FedClient {
    pub fn new(redis_url: &str) -> Result<Self, CalendarEventSourceError> {
        let http = OfficialHttpClient::new(redis_url).map_err(CalendarEventSourceError::Failed)?;
        Ok(Self { http })
    }
}

#[async_trait]
impl CalendarEventSource for FedClient {
    fn source(&self) -> &str {
        SOURCE
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let html = self.http.get_text(CALENDAR_URL).await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to fetch Federal Reserve calendar: {error}"
            ))
        })?;
        parser::parse_calendar_event_batch(&html, today)
    }
}
