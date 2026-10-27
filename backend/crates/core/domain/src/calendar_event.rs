use chrono::{DateTime, NaiveDate, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarEventCategory {
    Indicator,
    CentralBank,
    Earnings,
}

impl CalendarEventCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Indicator => "indicator",
            Self::CentralBank => "central_bank",
            Self::Earnings => "earnings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarEventTimeOfDay {
    PreMarket,
    PostMarket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarEventReferenceKind {
    EarningsSummary,
    IndividualDocument,
    OfficialListing,
}

impl CalendarEventReferenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EarningsSummary => "earnings_summary",
            Self::IndividualDocument => "individual_document",
            Self::OfficialListing => "official_listing",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "earnings_summary" => Some(Self::EarningsSummary),
            "individual_document" => Some(Self::IndividualDocument),
            "official_listing" => Some(Self::OfficialListing),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventReference {
    pub event_source: String,
    pub event_external_id: String,
    pub kind: CalendarEventReferenceKind,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventReadModel {
    pub event: CalendarEvent,
    pub references: Vec<CalendarEventReference>,
    pub has_matching_earnings_summary: bool,
}

impl From<CalendarEvent> for CalendarEventReadModel {
    fn from(event: CalendarEvent) -> Self {
        Self {
            event,
            references: Vec::new(),
            has_matching_earnings_summary: false,
        }
    }
}

impl CalendarEventTimeOfDay {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreMarket => "pre_market",
            Self::PostMarket => "post_market",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent {
    pub source: String,
    pub external_id: String,
    pub category: CalendarEventCategory,
    pub country: String,
    pub title: String,
    pub stock_id: Option<String>,
    pub fiscal_period: Option<String>,
    pub event_date: NaiveDate,
    pub event_at: Option<DateTime<Utc>>,
    pub time_of_day: Option<CalendarEventTimeOfDay>,
}
