use chrono::{LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Asia::Tokyo;
use chrono_tz::Europe;
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use scraper::{Html, Selector};

const SOURCE: &str = "ecb";
const COUNTRY: &str = "EU";
// ECB の会合カレンダーには時刻がないため、政策決定公表の慣例時刻を使う。
const POLICY_DECISION_ANNOUNCEMENT_TIME_LOCAL: (u32, u32) = (14, 15);

pub(crate) fn parse_calendar_event_batch(
    html: &str,
    today: NaiveDate,
) -> Result<CalendarEventBatch, CalendarEventSourceError> {
    let (mut events, to) = parse_calendar_events(html)?;
    events.retain(|event| event.event_date >= today);
    if to < today {
        return Err(parse_error("ECB calendar does not cover any future dates"));
    }

    Ok(CalendarEventBatch {
        date_range: DateRange { from: today, to },
        events,
    })
}

pub(crate) fn parse_calendar_events(
    html: &str,
) -> Result<(Vec<CalendarEvent>, NaiveDate), CalendarEventSourceError> {
    let document = Html::parse_document(html);
    let calendar_selector = selector("div.definition-list dl")?;
    let date_selector = selector("dt")?;
    let title_selector = selector("dd")?;
    let Some(calendar) = document.select(&calendar_selector).next() else {
        return Err(parse_error("ECB calendar definition list was not found"));
    };

    let dates = calendar.select(&date_selector).collect::<Vec<_>>();
    let titles = calendar.select(&title_selector).collect::<Vec<_>>();
    if dates.is_empty() || dates.len() != titles.len() {
        return Err(parse_error("ECB calendar entries were incomplete"));
    }

    let mut events = Vec::new();
    let mut coverage_end = None;
    for (date_node, title_node) in dates.into_iter().zip(titles) {
        let date_text = date_node.text().collect::<String>();
        let date = NaiveDate::parse_from_str(date_text.trim(), "%d/%m/%Y").map_err(|error| {
            parse_error(format!("invalid ECB calendar date {date_text:?}: {error}"))
        })?;
        coverage_end = Some(coverage_end.map_or(date, |current: NaiveDate| current.max(date)));

        let title = title_node
            .text()
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if is_monetary_policy_meeting(&title) {
            events.push(parse_meeting(date, title)?);
        }
    }

    let coverage_end =
        coverage_end.ok_or_else(|| parse_error("ECB calendar contained no dated entries"))?;
    Ok((events, coverage_end))
}

fn selector(value: &str) -> Result<Selector, CalendarEventSourceError> {
    Selector::parse(value)
        .map_err(|error| parse_error(format!("invalid ECB calendar selector {value}: {error}")))
}

fn is_monetary_policy_meeting(title: &str) -> bool {
    let normalized = title.to_ascii_lowercase();
    normalized.starts_with("governing council of the ecb:")
        && normalized.contains("monetary policy meeting")
        && !normalized.contains("non-monetary policy meeting")
}

fn parse_meeting(
    date: NaiveDate,
    title: String,
) -> Result<CalendarEvent, CalendarEventSourceError> {
    let event_at = if title
        .to_ascii_lowercase()
        .contains("followed by press conference")
    {
        let local_time = NaiveTime::from_hms_opt(
            POLICY_DECISION_ANNOUNCEMENT_TIME_LOCAL.0,
            POLICY_DECISION_ANNOUNCEMENT_TIME_LOCAL.1,
            0,
        )
        .ok_or_else(|| parse_error("invalid ECB policy decision announcement time"))?;
        let local_datetime = date.and_time(local_time);
        let local_datetime = match Europe::Berlin.from_local_datetime(&local_datetime) {
            LocalResult::Single(datetime) => datetime,
            LocalResult::Ambiguous(_, _) => {
                return Err(parse_error(
                    "ECB policy decision announcement time was ambiguous in Europe/Berlin",
                ));
            }
            LocalResult::None => {
                return Err(parse_error(
                    "ECB policy decision announcement time did not exist in Europe/Berlin",
                ));
            }
        };
        Some(local_datetime.with_timezone::<Utc>(&Utc))
    } else {
        None
    };
    let event_date = event_at
        .map(|event_at| event_at.with_timezone(&Tokyo).date_naive())
        .unwrap_or(date);
    Ok(CalendarEvent {
        source: SOURCE.to_owned(),
        external_id: date.format("%Y-%m-%d").to_string(),
        category: CalendarEventCategory::CentralBank,
        country: COUNTRY.to_owned(),
        title,
        stock_id: None,
        fiscal_period: None,
        event_date,
        event_at,
        time_of_day: None,
    })
}

fn parse_error(message: impl Into<String>) -> CalendarEventSourceError {
    CalendarEventSourceError::Failed(message.into())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::{calendar::source::CalendarEventBatch, daily_bar_source::DateRange};
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};

    use super::{parse_calendar_event_batch, parse_calendar_events};

    const CALENDAR_FIXTURE: &str = include_str!("fixtures/calendar.html");

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn utc_timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn event(
        external_id: &str,
        title: &str,
        event_date: NaiveDate,
        event_at: Option<DateTime<Utc>>,
    ) -> CalendarEvent {
        CalendarEvent {
            source: "ecb".to_owned(),
            external_id: external_id.to_owned(),
            category: CalendarEventCategory::CentralBank,
            country: "EU".to_owned(),
            title: title.to_owned(),
            stock_id: None,
            fiscal_period: None,
            event_date,
            event_at,
            time_of_day: None,
        }
    }

    #[test]
    fn parse_calendar_events_sets_times_only_for_decision_days_and_ignores_other_meetings() {
        assert_eq!(
            parse_calendar_events(CALENDAR_FIXTURE).expect("parse calendar"),
            (
                vec![
                    event(
                        "2042-01-17",
                        "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 1)",
                        date(2042, 1, 17),
                        None,
                    ),
                    event(
                        "2042-01-18",
                        "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 2), followed by press conference",
                        date(2042, 1, 18),
                        Some(utc_timestamp("2042-01-18T13:15:00Z")),
                    ),
                    event(
                        "2042-02-01",
                        "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 2), followed by press conference",
                        date(2042, 2, 1),
                        Some(utc_timestamp("2042-02-01T13:15:00Z")),
                    ),
                    event(
                        "2042-07-15",
                        "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 2), followed by press conference",
                        date(2042, 7, 15),
                        Some(utc_timestamp("2042-07-15T12:15:00Z")),
                    ),
                ],
                date(2042, 11, 20),
            ),
        );
    }

    #[test]
    fn parse_calendar_event_batch_filters_past_events_and_sets_coverage_range() {
        assert_eq!(
            parse_calendar_event_batch(CALENDAR_FIXTURE, date(2042, 2, 1))
                .expect("parse calendar batch"),
            CalendarEventBatch {
                date_range: DateRange {
                    from: date(2042, 2, 1),
                    to: date(2042, 11, 20),
                },
                events: vec![
                    event(
                        "2042-02-01",
                        "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 2), followed by press conference",
                        date(2042, 2, 1),
                        Some(utc_timestamp("2042-02-01T13:15:00Z")),
                    ),
                    event(
                        "2042-07-15",
                        "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 2), followed by press conference",
                        date(2042, 7, 15),
                        Some(utc_timestamp("2042-07-15T12:15:00Z")),
                    ),
                ],
            },
        );
    }

    #[test]
    fn parse_calendar_event_batch_rejects_stale_coverage() {
        assert_eq!(
            parse_calendar_event_batch(CALENDAR_FIXTURE, date(2042, 12, 1))
                .map(|_| "ok".to_owned())
                .map_err(|error| error.to_string()),
            Err(
                "calendar event source error: ECB calendar does not cover any future dates"
                    .to_owned()
            ),
        );
    }
}
