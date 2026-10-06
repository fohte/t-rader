use chrono::{DateTime, NaiveDate, Utc};
use core_application::calendar::read_use_cases::{CalendarEventReadItem, CalendarEventReadResult};
use core_domain::calendar_event::CalendarEvent;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalendarEventResponse {
    Event {
        source: String,
        external_id: String,
        category: String,
        country: String,
        title: String,
        stock_id: Option<String>,
        fiscal_period: Option<String>,
        event_date: NaiveDate,
        event_at: Option<DateTime<Utc>>,
        time_of_day: Option<String>,
    },
    OtherEarningsSummary {
        country: String,
        event_date: NaiveDate,
        count: usize,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarEventsResponse {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub events: Vec<CalendarEventResponse>,
}

impl From<CalendarEventReadResult> for CalendarEventsResponse {
    fn from(result: CalendarEventReadResult) -> Self {
        Self {
            from: result.date_range.from,
            to: result.date_range.to,
            events: result.events.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CalendarEventReadItem> for CalendarEventResponse {
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

impl From<CalendarEvent> for CalendarEventResponse {
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
