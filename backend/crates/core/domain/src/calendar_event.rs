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
