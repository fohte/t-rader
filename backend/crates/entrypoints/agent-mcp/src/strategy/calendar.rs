use chrono::NaiveDate;
use core_application::calendar::read_use_cases::{
    CalendarEventReadItem, CalendarEventReadUseCaseError,
};
use core_application::strategy_scope::StrategyScope;
use core_domain::calendar_event::CalendarEvent;
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{StrategyServer, internal_error, invalid_params};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ReadCalendarParams {
    /// 取得開始日 (YYYY-MM-DD)。省略時は今週の月曜日 (JST)
    pub from: Option<NaiveDate>,
    /// 取得終了日 (YYYY-MM-DD)。省略時は今週の日曜日 (JST)
    pub to: Option<NaiveDate>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct ReadCalendarResult {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub events: Vec<CalendarEventDto>,
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalendarEventDto {
    Event {
        source: String,
        external_id: String,
        category: String,
        country: String,
        title: String,
        stock_id: Option<String>,
        fiscal_period: Option<String>,
        event_date: NaiveDate,
        event_at: Option<chrono::DateTime<chrono::Utc>>,
        time_of_day: Option<String>,
    },
    OtherEarningsSummary {
        country: String,
        event_date: NaiveDate,
        count: usize,
    },
}

impl StrategyServer {
    pub(crate) async fn read_calendar_inner(
        &self,
        scope: StrategyScope,
        params: ReadCalendarParams,
    ) -> Result<ReadCalendarResult, McpError> {
        let result = self
            .dependencies
            .calendar_event_reads
            .list_events(params.from, params.to, Some(scope))
            .await
            .map_err(map_calendar_read_error)?;

        Ok(ReadCalendarResult {
            from: result.date_range.from,
            to: result.date_range.to,
            events: result.events.into_iter().map(Into::into).collect(),
        })
    }
}

impl From<CalendarEventReadItem> for CalendarEventDto {
    fn from(item: CalendarEventReadItem) -> Self {
        match item {
            CalendarEventReadItem::Event(event) => event.into(),
            CalendarEventReadItem::OtherEarningsSummary {
                country,
                event_date,
                count,
            } => Self::OtherEarningsSummary {
                country,
                event_date,
                count,
            },
        }
    }
}

impl From<CalendarEvent> for CalendarEventDto {
    fn from(event: CalendarEvent) -> Self {
        Self::Event {
            source: event.source,
            external_id: event.external_id,
            category: event.category.as_str().to_owned(),
            country: event.country,
            title: event.title,
            stock_id: event.stock_id,
            fiscal_period: event.fiscal_period,
            event_date: event.event_date,
            event_at: event.event_at,
            time_of_day: event.time_of_day.map(|value| value.as_str().to_owned()),
        }
    }
}

fn map_calendar_read_error(error: CalendarEventReadUseCaseError) -> McpError {
    match error {
        error @ CalendarEventReadUseCaseError::InvalidDateRange => {
            invalid_params(error.to_string())
        }
        error => {
            tracing::error!(error = %error, "strategy mcp calendar request failed");
            internal_error(format!("calendar request failed: {error}"))
        }
    }
}
