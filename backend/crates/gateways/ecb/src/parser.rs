use chrono::{DateTime, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Asia::Tokyo;
use chrono_tz::Europe;
use core_application::calendar::source::CalendarEventSourceError;
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use scraper::{Html, Selector};

const SOURCE: &str = "ecb";
const COUNTRY: &str = "EU";
// ECB の会合カレンダーには時刻がないため、慣例の現地時刻を使う。
const MEETING_TIME_LOCAL: (u32, u32) = (14, 15);

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

        let title = title_node.text().collect::<String>().trim().to_owned();
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
    let local_time = NaiveTime::from_hms_opt(MEETING_TIME_LOCAL.0, MEETING_TIME_LOCAL.1, 0)
        .ok_or_else(|| parse_error("invalid ECB meeting time"))?;
    let local_datetime = date.and_time(local_time);
    let local_datetime = match Europe::Berlin.from_local_datetime(&local_datetime) {
        LocalResult::Single(datetime) => datetime,
        LocalResult::Ambiguous(_, _) => {
            return Err(parse_error(
                "ECB meeting time was ambiguous in Europe/Berlin",
            ));
        }
        LocalResult::None => {
            return Err(parse_error(
                "ECB meeting time did not exist in Europe/Berlin",
            ));
        }
    };
    let event_at: DateTime<Utc> = local_datetime.with_timezone(&Utc);
    Ok(CalendarEvent {
        source: SOURCE.to_owned(),
        external_id: date.format("%Y-%m-%d").to_string(),
        category: CalendarEventCategory::CentralBank,
        country: COUNTRY.to_owned(),
        title,
        stock_id: None,
        fiscal_period: None,
        event_date: event_at.with_timezone(&Tokyo).date_naive(),
        event_at: Some(event_at),
        time_of_day: None,
    })
}

fn parse_error(message: impl Into<String>) -> CalendarEventSourceError {
    CalendarEventSourceError::Failed(message.into())
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, Utc};
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};

    use super::parse_calendar_events;

    const CALENDAR_FIXTURE: &str = include_str!("fixtures/calendar.html");

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn utc_timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn parse_calendar_events_converts_winter_and_summer_meetings_and_ignores_other_meetings() {
        assert_eq!(
            parse_calendar_events(CALENDAR_FIXTURE).expect("parse calendar"),
            (
                vec![
                CalendarEvent {
                    source: "ecb".to_owned(),
                    external_id: "2042-01-17".to_owned(),
                    category: CalendarEventCategory::CentralBank,
                    country: "EU".to_owned(),
                    title: "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 1)".to_owned(),
                    stock_id: None,
                    fiscal_period: None,
                    event_date: date(2042, 1, 17),
                    event_at: Some(utc_timestamp("2042-01-17T13:15:00Z")),
                    time_of_day: None,
                },
                CalendarEvent {
                    source: "ecb".to_owned(),
                    external_id: "2042-07-15".to_owned(),
                    category: CalendarEventCategory::CentralBank,
                    country: "EU".to_owned(),
                    title: "Governing Council of the ECB: monetary policy meeting in Exampleburgh (Day 2), followed by press conference".to_owned(),
                    stock_id: None,
                    fiscal_period: None,
                    event_date: date(2042, 7, 15),
                    event_at: Some(utc_timestamp("2042-07-15T12:15:00Z")),
                    time_of_day: None,
                },
                ],
                date(2042, 11, 20),
            ),
        );
    }
}
