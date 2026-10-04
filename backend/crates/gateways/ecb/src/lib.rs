//! ECB の会合カレンダーから金融政策決定会合を取得する gateway。

mod parser;

use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::source::{
    CalendarEventBatch, CalendarEventSource, CalendarEventSourceError,
};

const CALENDAR_URL: &str = "https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

pub struct EcbClient {
    http: reqwest::Client,
}

impl EcbClient {
    pub fn new() -> Result<Self, CalendarEventSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader/0.1 (calendar event source)")
            .build()
            .map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "failed to initialize ECB HTTP client: {error}"
                ))
            })?;

        Ok(Self { http })
    }
}

#[async_trait]
impl CalendarEventSource for EcbClient {
    fn source(&self) -> &str {
        "ecb"
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let response = self.http.get(CALENDAR_URL).send().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!("failed to fetch ECB calendar: {error}"))
        })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CalendarEventSourceError::Failed(format!(
                "ECB calendar returned HTTP status {status}"
            )));
        }

        let html = response.text().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!("failed to read ECB calendar: {error}"))
        })?;
        parser::parse_calendar_event_batch(&html, today)
    }
}
