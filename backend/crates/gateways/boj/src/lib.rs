//! 日本銀行の会合日程と公表予定を取得する gateway。

mod macro_references;
mod official_http;
mod parser;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_application::calendar::source::{
    CalendarEventBatch, CalendarEventSource, CalendarEventSourceError,
};
use official_http::OfficialHttpClient;

pub use macro_references::BojMacroReferenceSource;

const MEETING_URL: &str = "https://www.boj.or.jp/mopo/mpmsche_minu/index.htm";
const PUBLICATION_URL: &str = "https://www.boj.or.jp/about/calendar/index.htm";

/// 日本銀行の公開ページから会合予定を取得する。
pub struct BojClient {
    http: OfficialHttpClient,
}

impl BojClient {
    pub fn new(redis_url: &str) -> Result<Self, CalendarEventSourceError> {
        let http = OfficialHttpClient::new(redis_url).map_err(CalendarEventSourceError::Failed)?;
        Ok(Self { http })
    }

    async fn fetch_page(
        &self,
        page_name: &str,
        url: &str,
    ) -> Result<String, CalendarEventSourceError> {
        self.http.get_text(url).await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to fetch BOJ {page_name} page: {error}"
            ))
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
        let meeting_html = self.fetch_page("meeting schedule", MEETING_URL).await?;
        let publication_html = self
            .fetch_page("publication schedule", PUBLICATION_URL)
            .await?;

        parser::parse_calendar_event_batch(&meeting_html, &publication_html, today)
    }
}
