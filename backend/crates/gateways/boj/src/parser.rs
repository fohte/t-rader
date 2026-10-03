use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Asia::Tokyo;
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use scraper::{Html, Selector, element_ref::ElementRef};

const SOURCE: &str = "boj";
const COUNTRY: &str = "JP";
const MEETING_TITLE: &str = "金融政策決定会合";
const TANKAN_RELEASE_TIME: (u32, u32) = (8, 50);

struct JapaneseDateParts {
    year: Option<i32>,
    month: Option<u32>,
    day: u32,
}

pub(crate) fn parse_calendar_event_batch(
    meeting_html: &str,
    publication_html: &str,
    today: NaiveDate,
) -> Result<CalendarEventBatch, CalendarEventSourceError> {
    let mut events = parse_meeting_events(meeting_html, today)?;
    let (date_range, publication_events) = parse_publication_events(publication_html, today)?;
    events.extend(publication_events);

    Ok(CalendarEventBatch { date_range, events })
}

fn parse_meeting_events(
    html: &str,
    today: NaiveDate,
) -> Result<Vec<CalendarEvent>, CalendarEventSourceError> {
    let document = Html::parse_document(html);
    let headings = selector("#contents h2")?;
    let rows = selector("tbody tr")?;
    let cells = selector("td")?;
    let mut parsed_rows = 0;
    let mut events = Vec::new();

    for heading in document.select(&headings) {
        let Some(id) = heading.value().attr("id") else {
            continue;
        };
        let Some(year_text) = id.strip_prefix('p') else {
            continue;
        };
        let Ok(year) = year_text.parse::<i32>() else {
            continue;
        };
        let table_selector = selector(&format!("#{id} + .tbl-box table"))?;
        let Some(table) = document.select(&table_selector).next() else {
            return Err(parse_error(format!(
                "BOJ meeting table for {year} was not found"
            )));
        };

        for row in table.select(&rows) {
            let Some(first_cell) = row.select(&cells).next() else {
                return Err(parse_error(format!(
                    "BOJ meeting row for {year} has no date cell"
                )));
            };
            let date_text = text(&first_cell);
            let mut previous_date = None;
            let meeting_dates = date_text
                .split('・')
                .map(|date_text| {
                    let parts = parse_japanese_date_parts(date_text).ok_or_else(|| {
                        parse_error(format!("invalid BOJ meeting date: {date_text}"))
                    })?;
                    let date = date_from_parts(parts, today, previous_date, Some(year))
                        .ok_or_else(|| {
                            parse_error(format!("invalid BOJ meeting date: {date_text}"))
                        })?;
                    previous_date = Some(date);
                    Ok(date)
                })
                .collect::<Result<Vec<_>, CalendarEventSourceError>>()?;
            parsed_rows += 1;

            for (day_index, event_date) in meeting_dates.iter().copied().enumerate() {
                if event_date >= today {
                    events.push(CalendarEvent {
                        source: SOURCE.to_owned(),
                        external_id: format!("meeting-{event_date}"),
                        category: CalendarEventCategory::CentralBank,
                        country: COUNTRY.to_owned(),
                        title: if meeting_dates.len() == 1 {
                            MEETING_TITLE.to_owned()
                        } else {
                            format!("{MEETING_TITLE} ({}日目)", day_index + 1)
                        },
                        stock_id: None,
                        fiscal_period: None,
                        event_date,
                        event_at: None,
                        time_of_day: None,
                    });
                }
            }
        }
    }

    if parsed_rows == 0 {
        return Err(parse_error("BOJ meeting schedule contained no dated rows"));
    }

    Ok(events)
}

fn parse_publication_events(
    html: &str,
    today: NaiveDate,
) -> Result<(DateRange, Vec<CalendarEvent>), CalendarEventSourceError> {
    let document = Html::parse_document(html);
    let table_selector = selector("#p01 + .tbl-box table")?;
    let rows = selector("tbody tr")?;
    let cells = selector("td")?;
    let table = document
        .select(&table_selector)
        .next()
        .ok_or_else(|| parse_error("BOJ publication schedule table was not found"))?;
    let mut previous_date = None;
    let mut last_date = None;
    let mut events = Vec::new();
    let mut parsed_rows = 0;

    for row in table.select(&rows) {
        let row_cells = row.select(&cells).collect::<Vec<_>>();
        if row_cells.len() < 5 {
            return Err(parse_error("BOJ publication row has fewer than five cells"));
        }

        let date_text = text(&row_cells[0]);
        let event_date = if date_text.trim().is_empty() {
            previous_date
                .ok_or_else(|| parse_error("BOJ publication row has no date to inherit"))?
        } else {
            let parts = parse_japanese_date_parts(&date_text)
                .ok_or_else(|| parse_error(format!("invalid BOJ publication date: {date_text}")))?;
            date_from_parts(parts, today, previous_date, None)
                .ok_or_else(|| parse_error(format!("invalid BOJ publication date: {date_text}")))?
        };
        previous_date = Some(event_date);
        last_date =
            Some(last_date.map_or(event_date, |current: NaiveDate| current.max(event_date)));
        parsed_rows += 1;

        let title = text(&row_cells[4]);
        if event_date >= today && is_tankan_title(&title) {
            events.push(CalendarEvent {
                source: SOURCE.to_owned(),
                external_id: format!("tankan-{event_date}"),
                category: CalendarEventCategory::CentralBank,
                country: COUNTRY.to_owned(),
                title,
                stock_id: None,
                fiscal_period: None,
                event_date,
                event_at: Some(tankan_release_at(event_date)?),
                time_of_day: None,
            });
        }
    }

    if parsed_rows == 0 {
        return Err(parse_error(
            "BOJ publication schedule contained no dated rows",
        ));
    }

    let to = last_date.ok_or_else(|| parse_error("BOJ publication schedule has no end date"))?;
    if to < today {
        return Err(parse_error(
            "BOJ publication schedule does not cover any future dates",
        ));
    }
    let date_range = DateRange { from: today, to };
    Ok((date_range, events))
}

fn parse_japanese_date_parts(value: &str) -> Option<JapaneseDateParts> {
    let (year, date_text) = match value.split_once('年') {
        Some((year_text, rest)) => (Some(i32::try_from(trailing_number(year_text)?).ok()?), rest),
        None => (None, value),
    };
    let before_day = date_text.split_once('日')?.0;
    let (month, day_text) = match before_day.split_once('月') {
        Some((month_text, rest)) => (Some(trailing_number(month_text)?), rest),
        None => (None, before_day),
    };

    Some(JapaneseDateParts {
        year,
        month,
        day: trailing_number(day_text)?,
    })
}

fn date_from_parts(
    parts: JapaneseDateParts,
    today: NaiveDate,
    previous_date: Option<NaiveDate>,
    fallback_year: Option<i32>,
) -> Option<NaiveDate> {
    let month = parts
        .month
        .or_else(|| previous_date.map(|date| date.month()))?;
    let year = parts
        .year
        .or(fallback_year)
        .unwrap_or_else(|| infer_year(month, today, previous_date));

    NaiveDate::from_ymd_opt(year, month, parts.day)
}

fn infer_year(month: u32, today: NaiveDate, previous_date: Option<NaiveDate>) -> i32 {
    let (reference_year, reference_month) = previous_date
        .map(|date| (date.year(), date.month()))
        .unwrap_or((today.year(), today.month()));

    if month < reference_month && reference_month - month > 6 {
        reference_year + 1
    } else if month > reference_month && month - reference_month > 6 {
        reference_year - 1
    } else {
        reference_year
    }
}

fn trailing_number(value: &str) -> Option<u32> {
    let digits = value
        .trim_end()
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.chars().rev().collect::<String>().parse().ok()
}

fn is_tankan_title(title: &str) -> bool {
    title.contains("短観") || title.contains("短期経済観測")
}

fn tankan_release_at(
    event_date: NaiveDate,
) -> Result<chrono::DateTime<Utc>, CalendarEventSourceError> {
    let local_time = event_date
        .and_hms_opt(TANKAN_RELEASE_TIME.0, TANKAN_RELEASE_TIME.1, 0)
        .ok_or_else(|| parse_error("invalid BOJ tankan release time"))?;
    Tokyo
        .from_local_datetime(&local_time)
        .single()
        .map(|date_time| date_time.with_timezone(&Utc))
        .ok_or_else(|| parse_error("BOJ tankan release time could not be converted to UTC"))
}

fn selector(value: &str) -> Result<Selector, CalendarEventSourceError> {
    Selector::parse(value)
        .map_err(|error| parse_error(format!("invalid HTML selector {value}: {error}")))
}

fn text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_error(message: impl Into<String>) -> CalendarEventSourceError {
    CalendarEventSourceError::Failed(message.into())
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use core_domain::calendar_event::CalendarEvent;

    use super::*;

    #[test]
    fn parses_future_events_and_keeps_meetings_beyond_the_publication_range() {
        let today = NaiveDate::from_ymd_opt(2099, 8, 1).expect("valid date");
        let actual = parse_calendar_event_batch(
            include_str!("fixtures/meeting_schedule.html"),
            include_str!("fixtures/publication_schedule.html"),
            today,
        )
        .expect("fixture pages parse");

        assert_eq!(
            actual,
            CalendarEventBatch {
                date_range: DateRange {
                    from: NaiveDate::from_ymd_opt(2099, 8, 1).expect("valid date"),
                    to: NaiveDate::from_ymd_opt(2099, 9, 4).expect("valid date"),
                },
                events: vec![
                    event(
                        "meeting-2099-08-18",
                        "金融政策決定会合 (1日目)",
                        date(2099, 8, 18),
                        None,
                    ),
                    event(
                        "meeting-2099-08-19",
                        "金融政策決定会合 (2日目)",
                        date(2099, 8, 19),
                        None,
                    ),
                    event(
                        "meeting-2099-10-06",
                        "金融政策決定会合 (1日目)",
                        date(2099, 10, 6),
                        None,
                    ),
                    event(
                        "meeting-2099-10-07",
                        "金融政策決定会合 (2日目)",
                        date(2099, 10, 7),
                        None,
                    ),
                    event(
                        "meeting-2100-01-20",
                        "金融政策決定会合 (1日目)",
                        date(2100, 1, 20),
                        None,
                    ),
                    event(
                        "meeting-2100-01-21",
                        "金融政策決定会合 (2日目)",
                        date(2100, 1, 21),
                        None,
                    ),
                    event(
                        "tankan-2099-08-21",
                        "架空の短観公表",
                        date(2099, 8, 21),
                        Some(
                            Utc.with_ymd_and_hms(2099, 8, 20, 23, 50, 0)
                                .single()
                                .expect("valid UTC time")
                        ),
                    ),
                ],
            },
        );
    }

    fn event(
        external_id: &str,
        title: &str,
        event_date: NaiveDate,
        event_at: Option<chrono::DateTime<Utc>>,
    ) -> CalendarEvent {
        CalendarEvent {
            source: SOURCE.to_owned(),
            external_id: external_id.to_owned(),
            category: CalendarEventCategory::CentralBank,
            country: COUNTRY.to_owned(),
            title: title.to_owned(),
            stock_id: None,
            fiscal_period: None,
            event_date,
            event_at,
            time_of_day: None,
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }
}
