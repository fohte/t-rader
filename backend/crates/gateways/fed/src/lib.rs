//! FRB の FOMC 会合カレンダーを取得する gateway。

mod parser;

use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::source::{
    CalendarEventBatch, CalendarEventSource, CalendarEventSourceError,
};

const CALENDAR_URL: &str = "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const SOURCE: &str = "fed";

pub struct FedClient {
    http: reqwest::Client,
}

impl FedClient {
    pub fn new() -> Result<Self, CalendarEventSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader/0.1 (calendar event source)")
            .build()
            .map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "failed to initialize Federal Reserve calendar client: {error}"
                ))
            })?;

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
        let response = self.http.get(CALENDAR_URL).send().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to fetch Federal Reserve calendar: {error}"
            ))
        })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CalendarEventSourceError::Failed(format!(
                "Federal Reserve calendar returned HTTP status {status}"
            )));
        }

        let html = response.text().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to read Federal Reserve calendar: {error}"
            ))
        })?;
        parser::parse_calendar_event_batch(&html, today)
    }
}
