use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{Datelike, Months, NaiveDate};
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use reqwest::Url;

use crate::parser::parse_release_calendar;

const SOURCE: &str = "e_stat";
const RELEASE_CALENDAR_URL: &str = "https://www.e-stat.go.jp/release-calendar";
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
const MAX_LOOKAHEAD_MONTHS: u32 = 12;

pub struct EStatCalendarEventSource {
    http: reqwest::Client,
    base_url: Url,
}

impl EStatCalendarEventSource {
    pub fn new() -> Result<Self, CalendarEventSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader calendar importer")
            .build()
            .map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "failed to initialize e-Stat HTTP client: {error}"
                ))
            })?;
        let base_url = Url::parse(RELEASE_CALENDAR_URL).map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "invalid e-Stat release calendar URL: {error}"
            ))
        })?;

        Ok(Self { http, base_url })
    }

    async fn fetch_month(
        &self,
        month: NaiveDate,
    ) -> Result<crate::parser::ParsedCalendarMonth, CalendarEventSourceError> {
        let month_param = month.format("%Y%m").to_string();
        let mut url = self.base_url.clone();
        url.query_pairs_mut()
            .append_pair("page_month", &month_param);
        let response = self.http.get(url).send().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to fetch e-Stat release calendar for {month_param}: {error}"
            ))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(CalendarEventSourceError::Failed(format!(
                "e-Stat release calendar returned HTTP {status} for {month_param}"
            )));
        }
        let html = response.text().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to read e-Stat release calendar for {month_param}: {error}"
            ))
        })?;

        parse_release_calendar(&html, month).map_err(CalendarEventSourceError::Failed)
    }
}

#[async_trait]
impl CalendarEventSource for EStatCalendarEventSource {
    fn source(&self) -> &str {
        SOURCE
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let mut month = first_day_of_month(today)?;
        let mut events = BTreeMap::new();
        let mut range_to = None;

        for _ in 0..MAX_LOOKAHEAD_MONTHS {
            let parsed_month = self.fetch_month(month).await?;
            if let Some(period) = parsed_month
                .approximate_periods
                .iter()
                .filter(|period| period.to >= today)
                .min_by_key(|period| period.from)
            {
                let safe_to = period.from.pred_opt().ok_or_else(|| {
                    CalendarEventSourceError::Failed(
                        "e-Stat release calendar has no complete future date range".to_owned(),
                    )
                })?;
                if safe_to < today {
                    return Err(CalendarEventSourceError::Failed(format!(
                        "e-Stat release dates are approximate from {today}"
                    )));
                }
                events.extend(
                    parsed_month
                        .events
                        .into_iter()
                        .filter(|event| event.event_date <= safe_to)
                        .map(|event| (event.external_id.clone(), event)),
                );
                return Ok(CalendarEventBatch {
                    date_range: DateRange {
                        from: today,
                        to: safe_to,
                    },
                    events: events.into_values().collect(),
                });
            }

            events.extend(
                parsed_month
                    .events
                    .into_iter()
                    .filter(|event| event.event_date >= today)
                    .map(|event| (event.external_id.clone(), event)),
            );
            range_to = Some(last_day_of_month(month)?);
            month = next_month(month)?;
        }

        let to = range_to.ok_or_else(|| {
            CalendarEventSourceError::Failed(
                "e-Stat release calendar returned no complete month".to_owned(),
            )
        })?;
        Ok(CalendarEventBatch {
            date_range: DateRange { from: today, to },
            events: events.into_values().collect(),
        })
    }
}

fn first_day_of_month(date: NaiveDate) -> Result<NaiveDate, CalendarEventSourceError> {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).ok_or_else(|| {
        CalendarEventSourceError::Failed(format!("invalid month in requested date: {date}"))
    })
}

fn last_day_of_month(month: NaiveDate) -> Result<NaiveDate, CalendarEventSourceError> {
    next_month(month)?
        .pred_opt()
        .ok_or_else(|| CalendarEventSourceError::Failed(format!("invalid end of month: {month}")))
}

fn next_month(month: NaiveDate) -> Result<NaiveDate, CalendarEventSourceError> {
    month
        .checked_add_months(Months::new(1))
        .ok_or_else(|| CalendarEventSourceError::Failed(format!("month is out of range: {month}")))
}

#[cfg(test)]
mod tests {
    use super::{first_day_of_month, last_day_of_month, next_month};

    #[test]
    fn month_boundaries_cover_year_transition_and_leap_day() {
        let actual = (
            first_day_of_month(chrono::NaiveDate::from_ymd_opt(2026, 12, 13).expect("date")),
            last_day_of_month(chrono::NaiveDate::from_ymd_opt(2028, 2, 1).expect("date")),
            next_month(chrono::NaiveDate::from_ymd_opt(2026, 12, 1).expect("date")),
        );

        assert_eq!(
            actual,
            (
                Ok(chrono::NaiveDate::from_ymd_opt(2026, 12, 1).expect("date")),
                Ok(chrono::NaiveDate::from_ymd_opt(2028, 2, 29).expect("date")),
                Ok(chrono::NaiveDate::from_ymd_opt(2027, 1, 1).expect("date")),
            ),
        );
    }
}
