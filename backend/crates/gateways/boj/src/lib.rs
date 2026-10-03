//! 日本銀行の会合日程と公表予定を取得する gateway。

mod parser;

use std::time::Duration;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::source::{
    CalendarEventBatch, CalendarEventSource, CalendarEventSourceError,
};

const MEETING_URL: &str = "https://www.boj.or.jp/mopo/mpmsche_minu/index.htm";
const PUBLICATION_URL: &str = "https://www.boj.or.jp/about/calendar/index.htm";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

/// 日本銀行の公開ページから会合予定を取得する。
pub struct BojClient {
    http: reqwest::Client,
}

impl BojClient {
    pub fn new() -> Result<Self, CalendarEventSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader/0.1 (calendar event source)")
            .build()
            .map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "failed to initialize BOJ HTTP client: {error}"
                ))
            })?;

        Ok(Self { http })
    }

    async fn fetch_page(&self, url: &str) -> Result<String, CalendarEventSourceError> {
        let response = self.http.get(url).send().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!("failed to fetch BOJ page: {error}"))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(CalendarEventSourceError::Failed(format!(
                "BOJ page returned HTTP {status}"
            )));
        }

        response.text().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!("failed to read BOJ page: {error}"))
        })
    }
}

#[async_trait]
impl CalendarEventSource for BojClient {
    fn source(&self) -> &str {
        "boj"
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let meeting_html = self.fetch_page(MEETING_URL).await?;
        let publication_html = self.fetch_page(PUBLICATION_URL).await?;

        parser::parse_calendar_event_batch(&meeting_html, &publication_html, today)
    }
}
