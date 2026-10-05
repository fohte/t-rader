use chrono::{LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::{America::New_York, Asia::Tokyo};
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use scraper::{ElementRef, Html, Selector};

use crate::SOURCE;

const COUNTRY: &str = "US";
const ANNOUNCEMENT_TIME: (u32, u32) = (14, 0);

pub(crate) fn parse_calendar_event_batch(
    html: &str,
    today: NaiveDate,
) -> Result<CalendarEventBatch, CalendarEventSourceError> {
    let (mut events, to) = parse_calendar_events(html)?;
    events.retain(|event| event.event_date >= today);
    if to < today {
        return Err(parse_error(
            "Federal Reserve calendar does not cover any future dates",
        ));
    }

    Ok(CalendarEventBatch {
        date_range: DateRange { from: today, to },
        events,
    })
}

fn parse_calendar_events(
    html: &str,
) -> Result<(Vec<CalendarEvent>, NaiveDate), CalendarEventSourceError> {
    let document = Html::parse_document(html);
    let panel_selector = selector("#article .panel.panel-default")?;
    let heading_selector = selector(".panel-heading h4")?;
    let meeting_selector = selector(".fomc-meeting")?;
    let month_selector = selector(".fomc-meeting__month")?;
    let date_selector = selector(".fomc-meeting__date")?;
    let panels = document.select(&panel_selector).collect::<Vec<_>>();
    if panels.is_empty() {
        return Err(parse_error(
            "Federal Reserve calendar year panels were not found",
        ));
    }

    let mut events = Vec::new();
    for panel in panels {
        let heading = select_text(panel, &heading_selector, "year heading")?;
        let year = parse_year(&heading)?;
        for meeting in panel.select(&meeting_selector) {
            let month = select_text(meeting, &month_selector, "meeting month")?;
            let date_text = select_text(meeting, &date_selector, "meeting date")?;
            if date_text.contains("(notation vote)") {
                continue;
            }
            let statement_date = parse_statement_date(year, &month, &date_text)?;
            events.push(parse_meeting(statement_date)?);
        }
    }

    if events.is_empty() {
        return Err(parse_error(
            "Federal Reserve calendar contained no meeting dates",
        ));
    }
    events.sort_by(|left, right| {
        (&left.event_date, &left.event_at, &left.external_id).cmp(&(
            &right.event_date,
            &right.event_at,
            &right.external_id,
        ))
    });
    let coverage_end = events
        .last()
        .map(|event| event.event_date)
        .ok_or_else(|| parse_error("Federal Reserve calendar contained no meeting dates"))?;

    Ok((events, coverage_end))
}

fn parse_year(heading: &str) -> Result<i32, CalendarEventSourceError> {
    let year = heading
        .trim()
        .strip_suffix(" FOMC Meetings")
        .ok_or_else(|| parse_error(format!("invalid Federal Reserve year heading: {heading}")))?;
    year.parse::<i32>()
        .map_err(|error| parse_error(format!("invalid Federal Reserve calendar year: {error}")))
}

fn parse_statement_date(
    year: i32,
    month_text: &str,
    date_text: &str,
) -> Result<NaiveDate, CalendarEventSourceError> {
    let mut months = month_text.split('/');
    let start_month_text = months.next().unwrap_or_default().trim();
    let start_month = parse_month(start_month_text)?;
    let explicit_end_month = months.next().map(str::trim).map(parse_month).transpose()?;
    if months.next().is_some() {
        return Err(parse_error(format!(
            "invalid Federal Reserve meeting month: {month_text}"
        )));
    }

    let date_range = date_text.trim().trim_end_matches('*').trim();
    let (start_day_text, end_day_text) = date_range
        .split_once('-')
        .ok_or_else(|| parse_error(format!("invalid Federal Reserve meeting date: {date_text}")))?;
    let start_day = parse_day(start_day_text, date_text)?;
    let end_day = parse_day(end_day_text, date_text)?;
    let end_month = explicit_end_month.unwrap_or_else(|| {
        if end_day < start_day {
            next_month(start_month)
        } else {
            start_month
        }
    });
    let end_year = if end_month < start_month {
        year.checked_add(1)
            .ok_or_else(|| parse_error("Federal Reserve meeting year exceeded supported range"))?
    } else {
        year
    };
    let start_date = NaiveDate::from_ymd_opt(year, start_month, start_day)
        .ok_or_else(|| parse_error(format!("invalid Federal Reserve meeting date: {date_text}")))?;
    let end_date = NaiveDate::from_ymd_opt(end_year, end_month, end_day)
        .ok_or_else(|| parse_error(format!("invalid Federal Reserve meeting date: {date_text}")))?;
    if end_date < start_date {
        return Err(parse_error(format!(
            "Federal Reserve meeting ends before it starts: {date_text}"
        )));
    }

    Ok(end_date)
}

fn parse_month(value: &str) -> Result<u32, CalendarEventSourceError> {
    match value.to_ascii_lowercase().as_str() {
        "jan" | "january" => Ok(1),
        "feb" | "february" => Ok(2),
        "mar" | "march" => Ok(3),
        "apr" | "april" => Ok(4),
        "may" => Ok(5),
        "jun" | "june" => Ok(6),
        "jul" | "july" => Ok(7),
        "aug" | "august" => Ok(8),
        "sep" | "sept" | "september" => Ok(9),
        "oct" | "october" => Ok(10),
        "nov" | "november" => Ok(11),
        "dec" | "december" => Ok(12),
        _ => Err(parse_error(format!(
            "invalid Federal Reserve month: {value}"
        ))),
    }
}

fn parse_day(value: &str, date_text: &str) -> Result<u32, CalendarEventSourceError> {
    value
        .trim()
        .parse::<u32>()
        .map_err(|error| parse_error(format!("invalid Federal Reserve date {date_text}: {error}")))
}

fn next_month(month: u32) -> u32 {
    if month == 12 { 1 } else { month + 1 }
}

fn parse_meeting(statement_date: NaiveDate) -> Result<CalendarEvent, CalendarEventSourceError> {
    let (hour, minute) = ANNOUNCEMENT_TIME;
    let announcement_time = NaiveTime::from_hms_opt(hour, minute, 0)
        .ok_or_else(|| parse_error("invalid Federal Reserve announcement time"))?;
    let local_datetime = statement_date.and_time(announcement_time);
    let local_datetime = match New_York.from_local_datetime(&local_datetime) {
        LocalResult::Single(datetime) => datetime,
        LocalResult::Ambiguous(_, _) => {
            return Err(parse_error(
                "Federal Reserve announcement time was ambiguous in America/New_York",
            ));
        }
        LocalResult::None => {
            return Err(parse_error(
                "Federal Reserve announcement time did not exist in America/New_York",
            ));
        }
    };
    let event_at = local_datetime.with_timezone::<Utc>(&Utc);
    let event_date = event_at.with_timezone(&Tokyo).date_naive();

    Ok(CalendarEvent {
        source: SOURCE.to_owned(),
        external_id: statement_date.format("%Y-%m-%d").to_string(),
        category: CalendarEventCategory::CentralBank,
        country: COUNTRY.to_owned(),
        title: "FOMC 声明".to_owned(),
        stock_id: None,
        fiscal_period: None,
        event_date,
        event_at: Some(event_at),
        time_of_day: None,
    })
}

fn selector(value: &str) -> Result<Selector, CalendarEventSourceError> {
    Selector::parse(value)
        .map_err(|error| parse_error(format!("invalid Federal Reserve selector {value}: {error}")))
}

fn select_text(
    element: ElementRef<'_>,
    selector: &Selector,
    field: &str,
) -> Result<String, CalendarEventSourceError> {
    element
        .select(selector)
        .next()
        .map(|node| {
            node.text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|text| !text.is_empty())
        .ok_or_else(|| parse_error(format!("Federal Reserve calendar {field} was missing")))
}

fn parse_error(message: impl Into<String>) -> CalendarEventSourceError {
    CalendarEventSourceError::Failed(message.into())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use core_application::{calendar::source::CalendarEventBatch, daily_bar_source::DateRange};
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};

    use super::parse_calendar_event_batch;

    const CALENDAR_FIXTURE: &str = include_str!("fixtures/calendar.html.fixture");

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn utc(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &date(year, month, day)
                .and_hms_opt(hour, 0, 0)
                .expect("valid time"),
        )
    }

    fn event(
        statement_date: NaiveDate,
        event_date: NaiveDate,
        event_at: DateTime<Utc>,
    ) -> CalendarEvent {
        CalendarEvent {
            source: "fed".to_owned(),
            external_id: statement_date.format("%Y-%m-%d").to_string(),
            category: CalendarEventCategory::CentralBank,
            country: "US".to_owned(),
            title: "FOMC 声明".to_owned(),
            stock_id: None,
            fiscal_period: None,
            event_date,
            event_at: Some(event_at),
            time_of_day: None,
        }
    }

    #[test]
    fn parses_official_meetings_and_converts_eastern_announcement_time_to_tokyo() {
        assert_eq!(
            parse_calendar_event_batch(CALENDAR_FIXTURE, date(2026, 10, 5)),
            Ok(CalendarEventBatch {
                date_range: DateRange {
                    from: date(2026, 10, 5),
                    to: date(2027, 12, 9),
                },
                events: vec![
                    event(
                        date(2026, 10, 28),
                        date(2026, 10, 29),
                        utc(2026, 10, 28, 18)
                    ),
                    event(date(2026, 12, 9), date(2026, 12, 10), utc(2026, 12, 9, 19)),
                    event(date(2027, 1, 27), date(2027, 1, 28), utc(2027, 1, 27, 19)),
                    event(date(2027, 3, 17), date(2027, 3, 18), utc(2027, 3, 17, 18)),
                    event(date(2027, 4, 28), date(2027, 4, 29), utc(2027, 4, 28, 18)),
                    event(date(2027, 6, 9), date(2027, 6, 10), utc(2027, 6, 9, 18)),
                    event(date(2027, 7, 28), date(2027, 7, 29), utc(2027, 7, 28, 18)),
                    event(date(2027, 9, 15), date(2027, 9, 16), utc(2027, 9, 15, 18)),
                    event(
                        date(2027, 10, 27),
                        date(2027, 10, 28),
                        utc(2027, 10, 27, 18)
                    ),
                    event(date(2027, 12, 8), date(2027, 12, 9), utc(2027, 12, 8, 19)),
                ],
            }),
        );
    }
}
