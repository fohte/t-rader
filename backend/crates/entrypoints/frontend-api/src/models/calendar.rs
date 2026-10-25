use chrono::{DateTime, NaiveDate, Utc};
use core_application::calendar::read_use_cases::{CalendarEventReadItem, CalendarEventReadResult};
use core_domain::calendar_event::{
    CalendarEvent, CalendarEventReadModel, CalendarEventReference, CalendarEventReferenceKind,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CalendarEventResponse {
    Event(Box<CalendarEventResponseEvent>),
    OtherEarningsSummary {
        country: String,
        event_date: NaiveDate,
        count: usize,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarEventResponseEvent {
    pub source: String,
    pub external_id: String,
    pub category: String,
    pub country: String,
    pub title: String,
    pub stock_id: Option<String>,
    pub fiscal_period: Option<String>,
    pub fiscal_period_type: Option<String>,
    pub event_date: NaiveDate,
    pub event_at: Option<DateTime<Utc>>,
    pub time_of_day: Option<String>,
    pub references: Vec<CalendarEventReferenceResponse>,
    pub has_matching_earnings_summary: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarEventsResponse {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub events: Vec<CalendarEventResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarOtherEarningsResponse {
    pub events: Vec<CalendarEventResponse>,
}

impl From<Vec<CalendarEventReadModel>> for CalendarOtherEarningsResponse {
    fn from(events: Vec<CalendarEventReadModel>) -> Self {
        Self {
            events: events.into_iter().map(Into::into).collect(),
        }
    }
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
        let fiscal_period_type = fiscal_period_type(&event);
        Self::Event(Box::new(CalendarEventResponseEvent {
            source: event.source,
            external_id: event.external_id,
            category: event.category.as_str().to_owned(),
            country: event.country,
            title: event.title,
            stock_id: event.stock_id,
            fiscal_period: event.fiscal_period,
            fiscal_period_type,
            event_date: event.event_date,
            event_at: event.event_at,
            time_of_day: event.time_of_day.map(|value| value.as_str().to_owned()),
            references: Vec::new(),
            has_matching_earnings_summary: false,
        }))
    }
}

impl From<CalendarEventReadModel> for CalendarEventResponse {
    fn from(read_model: CalendarEventReadModel) -> Self {
        let fiscal_period_type = fiscal_period_type(&read_model.event);
        Self::Event(Box::new(CalendarEventResponseEvent {
            source: read_model.event.source,
            external_id: read_model.event.external_id,
            category: read_model.event.category.as_str().to_owned(),
            country: read_model.event.country,
            title: read_model.event.title,
            stock_id: read_model.event.stock_id,
            fiscal_period: read_model.event.fiscal_period,
            fiscal_period_type,
            event_date: read_model.event.event_date,
            event_at: read_model.event.event_at,
            time_of_day: read_model
                .event
                .time_of_day
                .map(|value| value.as_str().to_owned()),
            references: read_model.references.into_iter().map(Into::into).collect(),
            has_matching_earnings_summary: read_model.has_matching_earnings_summary,
        }))
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CalendarEventReferenceResponse {
    pub kind: CalendarEventReferenceKindResponse,
    pub source: String,
    pub external_id: String,
    pub title: String,
    pub url: Option<String>,
    pub target_period_start: Option<NaiveDate>,
    pub target_period_end: Option<NaiveDate>,
    pub target_period_type: Option<String>,
    pub publication_evidence: Option<String>,
    pub publication_evidence_url: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CalendarEventReferenceKindResponse {
    EarningsSummary,
    IndividualDocument,
    OfficialListing,
}

impl From<CalendarEventReference> for CalendarEventReferenceResponse {
    fn from(reference: CalendarEventReference) -> Self {
        Self {
            kind: match reference.kind {
                CalendarEventReferenceKind::EarningsSummary => {
                    CalendarEventReferenceKindResponse::EarningsSummary
                }
                CalendarEventReferenceKind::IndividualDocument => {
                    CalendarEventReferenceKindResponse::IndividualDocument
                }
                CalendarEventReferenceKind::OfficialListing => {
                    CalendarEventReferenceKindResponse::OfficialListing
                }
            },
            source: reference.source,
            external_id: reference.external_id,
            title: reference.title,
            url: reference.url,
            target_period_start: reference.target_period_start,
            target_period_end: reference.target_period_end,
            target_period_type: reference.target_period_type,
            publication_evidence: reference.publication_evidence,
            publication_evidence_url: reference.publication_evidence_url,
            checked_at: reference.checked_at,
        }
    }
}

fn fiscal_period_type(event: &CalendarEvent) -> Option<String> {
    if event.category.as_str() != "earnings" || event.country != "JP" {
        return None;
    }
    let mut parts = event.external_id.split(':');
    let _code = parts.next()?;
    let period_type = parts.next()?;
    let period_end = parts.next()?;
    if parts.next().is_some()
        || event.fiscal_period.as_deref() != Some(period_end)
        || !matches!(period_type, "FY" | "1Q" | "2Q" | "3Q" | "4Q")
    {
        return None;
    }
    Some(period_type.to_owned())
}
