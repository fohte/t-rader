use std::collections::BTreeMap;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use scraper::{Html, Selector};

use crate::{SOURCE_NAME, month::last_day_of_month};

const JAPAN_OFFSET_SECONDS: i32 = 9 * 60 * 60;
const TARGET_STATISTICS: [(&str, &str); 6] = [
    ("00200573", "消費者物価指数"),
    ("00350300", "貿易統計"),
    ("00100409", "国民経済計算"),
    ("00550300", "鉱工業生産・出荷・在庫指数"),
    ("00100401", "機械受注統計調査"),
    ("00200531", "労働力調査"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApproximatePeriod {
    pub(crate) from: NaiveDate,
    pub(crate) to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedCalendarMonth {
    pub(crate) events: Vec<CalendarEvent>,
    pub(crate) approximate_periods: Vec<ApproximatePeriod>,
}

enum PublicationTiming {
    Exact {
        date: NaiveDate,
        time: Option<NaiveTime>,
    },
    Approximate(ApproximatePeriod),
}

pub(crate) fn parse_release_calendar(
    html: &str,
    month: NaiveDate,
) -> Result<ParsedCalendarMonth, String> {
    let document = Html::parse_document(html);
    let body_selector = selector(".stat-list-body")?;
    if document.select(&body_selector).next().is_none() {
        return Err("e-Stat release calendar is missing the announcement list".to_owned());
    }

    let row_selector = selector(".stat-list-body > li.stat-list-row")?;
    let date_selector = selector(".stat-announce-keisaiday")?;
    let announcement_selector = selector(".stat-announce-comment")?;
    let mut events = BTreeMap::new();
    let mut approximate_periods = Vec::new();

    for row in document.select(&row_selector) {
        let announcement = row
            .select(&announcement_selector)
            .next()
            .ok_or_else(|| "e-Stat announcement row is missing its content".to_owned())?;
        let stat_code = announcement
            .value()
            .attr("data-toukei_cd")
            .ok_or_else(|| "e-Stat announcement is missing its statistics code".to_owned())?;
        let Some((_, title)) = TARGET_STATISTICS
            .iter()
            .find(|(code, _)| *code == stat_code)
        else {
            continue;
        };
        let date_element = row
            .select(&date_selector)
            .next()
            .ok_or_else(|| "e-Stat announcement is missing its release date".to_owned())?;
        let date_text = text_content(date_element);
        let timing = parse_publication_timing(&date_text, month)?;
        match timing {
            PublicationTiming::Exact { date, time } => {
                let external_date = announcement
                    .value()
                    .attr("data-kensakukouhyou_date")
                    .ok_or_else(|| {
                        "e-Stat announcement is missing its publication identifier".to_owned()
                    })?;
                validate_publication_identifier(external_date, date, time)?;
                let external_id = format!("{stat_code}:{external_date}");
                let event = CalendarEvent {
                    source: SOURCE_NAME.to_owned(),
                    external_id: external_id.clone(),
                    category: CalendarEventCategory::Indicator,
                    country: "JP".to_owned(),
                    title: (*title).to_owned(),
                    stock_id: None,
                    fiscal_period: None,
                    event_date: date,
                    event_at: time.map(|time| publication_time(date, time)).transpose()?,
                    time_of_day: None,
                };
                events.insert(external_id, event);
            }
            PublicationTiming::Approximate(period) => approximate_periods.push(period),
        }
    }

    approximate_periods.sort_by_key(|period| period.from);
    Ok(ParsedCalendarMonth {
        events: events.into_values().collect(),
        approximate_periods,
    })
}

fn selector(value: &str) -> Result<Selector, String> {
    Selector::parse(value).map_err(|error| format!("invalid e-Stat HTML selector: {error}"))
}

fn text_content(element: scraper::ElementRef<'_>) -> String {
    element
        .text()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_publication_timing(value: &str, month: NaiveDate) -> Result<PublicationTiming, String> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let date_text = parts.first().copied().unwrap_or_default();
    if let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") {
        if date.year() != month.year() || date.month() != month.month() {
            return Err(format!(
                "e-Stat release date {date} does not belong to requested month {month}"
            ));
        }
        let time = match parts.as_slice() {
            [_] => None,
            [_, value] => Some(
                NaiveTime::parse_from_str(value, "%H:%M")
                    .map_err(|error| format!("invalid e-Stat release time '{value}': {error}"))?,
            ),
            _ => return Err(format!("invalid e-Stat release date: {value}")),
        };
        return Ok(PublicationTiming::Exact { date, time });
    }

    if parts.len() != 1 {
        return Err(format!("invalid e-Stat release date: {value}"));
    }
    parse_approximate_period(date_text, month)
        .map(PublicationTiming::Approximate)
        .ok_or_else(|| format!("unsupported e-Stat release date format: {value}"))
}

fn parse_approximate_period(value: &str, month: NaiveDate) -> Option<ApproximatePeriod> {
    let month_prefixes = [
        format!("{}-{:02}月", month.year(), month.month()),
        format!("{}-{}月", month.year(), month.month()),
        format!("{}年{}月", month.year(), month.month()),
    ];
    let suffix = month_prefixes
        .iter()
        .find_map(|prefix| value.strip_prefix(prefix))?;
    let (first_day, last_day) = match suffix {
        "" | "頃" => (1, last_day_of_month(month)?.day()),
        "上旬" => (1, 10),
        "中旬" => (11, 20),
        "下旬" => (21, last_day_of_month(month)?.day()),
        _ => return None,
    };
    Some(ApproximatePeriod {
        from: NaiveDate::from_ymd_opt(month.year(), month.month(), first_day)?,
        to: NaiveDate::from_ymd_opt(month.year(), month.month(), last_day)?,
    })
}

fn validate_publication_identifier(
    value: &str,
    date: NaiveDate,
    time: Option<NaiveTime>,
) -> Result<(), String> {
    let identifier = NaiveDateTime::parse_from_str(value, "%Y%m%d%H%M")
        .map_err(|error| format!("invalid e-Stat publication identifier '{value}': {error}"))?;
    if identifier.date() != date || time.is_some_and(|time| identifier.time() != time) {
        return Err(format!(
            "e-Stat publication identifier '{value}' does not match displayed release date"
        ));
    }
    Ok(())
}

fn publication_time(date: NaiveDate, time: NaiveTime) -> Result<DateTime<Utc>, String> {
    let local = date.and_time(time);
    let japan = FixedOffset::east_opt(JAPAN_OFFSET_SECONDS)
        .ok_or_else(|| "invalid JST offset".to_owned())?;
    japan
        .from_local_datetime(&local)
        .single()
        .map(|date_time| date_time.with_timezone(&Utc))
        .ok_or_else(|| format!("invalid e-Stat release datetime: {local}"))
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
    use rstest::rstest;

    use super::{ApproximatePeriod, ParsedCalendarMonth, parse_release_calendar};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn event(
        stat_code: &str,
        title: &str,
        publication_id: &str,
        event_date: NaiveDate,
        event_at: Option<chrono::DateTime<Utc>>,
    ) -> CalendarEvent {
        CalendarEvent {
            source: "e_stat".to_owned(),
            external_id: format!("{stat_code}:{publication_id}"),
            category: CalendarEventCategory::Indicator,
            country: "JP".to_owned(),
            title: title.to_owned(),
            stock_id: None,
            fiscal_period: None,
            event_date,
            event_at,
            time_of_day: None,
        }
    }

    fn utc_datetime(value: &str) -> chrono::DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(value)
            .expect("valid RFC 3339 datetime")
            .with_timezone(&Utc)
    }

    #[rstest]
    #[case::october_schedule(
        include_str!("../tests/fixtures/release-calendar-202610.html"),
        date(2026, 10, 1),
        ParsedCalendarMonth {
            events: vec![
                event("00100401", "機械受注統計調査", "202610150850", date(2026, 10, 15), Some(utc_datetime("2026-10-14T23:50:00Z"))),
                event("00200531", "労働力調査", "202610020830", date(2026, 10, 2), Some(utc_datetime("2026-10-01T23:30:00Z"))),
                event("00200531", "労働力調査", "202610300830", date(2026, 10, 30), Some(utc_datetime("2026-10-29T23:30:00Z"))),
                event("00200573", "消費者物価指数", "202610020830", date(2026, 10, 2), Some(utc_datetime("2026-10-01T23:30:00Z"))),
                event("00200573", "消費者物価指数", "202610230830", date(2026, 10, 23), Some(utc_datetime("2026-10-22T23:30:00Z"))),
                event("00200573", "消費者物価指数", "202610300830", date(2026, 10, 30), Some(utc_datetime("2026-10-29T23:30:00Z"))),
                event("00350300", "貿易統計", "202610290930", date(2026, 10, 29), Some(utc_datetime("2026-10-29T00:30:00Z"))),
                event("00550300", "鉱工業生産・出荷・在庫指数", "202610151330", date(2026, 10, 15), Some(utc_datetime("2026-10-15T04:30:00Z"))),
                event("00550300", "鉱工業生産・出荷・在庫指数", "202610300850", date(2026, 10, 30), Some(utc_datetime("2026-10-29T23:50:00Z"))),
            ],
            approximate_periods: vec![],
        },
    )]
    #[case::march_schedule_with_approximate_release(
        include_str!("../tests/fixtures/release-calendar-202703.html"),
        date(2027, 3, 1),
        ParsedCalendarMonth {
            events: vec![
                event("00100401", "機械受注統計調査", "202703180850", date(2027, 3, 18), Some(utc_datetime("2027-03-17T23:50:00Z"))),
                event("00100409", "国民経済計算", "202703090850", date(2027, 3, 9), Some(utc_datetime("2027-03-08T23:50:00Z"))),
                event("00200531", "労働力調査", "202703020830", date(2027, 3, 2), Some(utc_datetime("2027-03-01T23:30:00Z"))),
                event("00200531", "労働力調査", "202703300830", date(2027, 3, 30), Some(utc_datetime("2027-03-29T23:30:00Z"))),
                event("00200573", "消費者物価指数", "202703190830", date(2027, 3, 19), Some(utc_datetime("2027-03-18T23:30:00Z"))),
                event("00200573", "消費者物価指数", "202703260830", date(2027, 3, 26), Some(utc_datetime("2027-03-25T23:30:00Z"))),
                event("00350300", "貿易統計", "202703300930", date(2027, 3, 30), Some(utc_datetime("2027-03-30T00:30:00Z"))),
                event("00550300", "鉱工業生産・出荷・在庫指数", "202703191330", date(2027, 3, 19), Some(utc_datetime("2027-03-19T04:30:00Z"))),
                event("00550300", "鉱工業生産・出荷・在庫指数", "202703310850", date(2027, 3, 31), Some(utc_datetime("2027-03-30T23:50:00Z"))),
            ],
            approximate_periods: vec![ApproximatePeriod {
                from: date(2027, 3, 1),
                to: date(2027, 3, 31),
            }],
        },
    )]
    fn parses_saved_release_calendar_html(
        #[case] html: &str,
        #[case] month: NaiveDate,
        #[case] expected: ParsedCalendarMonth,
    ) {
        assert_eq!(parse_release_calendar(html, month), Ok(expected));
    }
}
