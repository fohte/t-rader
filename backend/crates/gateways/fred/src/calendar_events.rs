use async_trait::async_trait;
use chrono::{Months, NaiveDate, TimeZone, Utc};
use chrono_tz::{America::New_York, Asia::Tokyo};
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
use reqwest::Url;
use serde::Deserialize;

use crate::FredClient;

const RELEASE_DATES_LIMIT: usize = 10_000;
const LOOKAHEAD_MONTHS: u32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReleaseDefinition {
    id: i32,
    title: &'static str,
    category: CalendarEventCategory,
    hour: u32,
    minute: u32,
}

const RELEASES: [ReleaseDefinition; 6] = [
    ReleaseDefinition {
        id: 10,
        title: "消費者物価指数 (CPI)",
        category: CalendarEventCategory::Indicator,
        hour: 8,
        minute: 30,
    },
    ReleaseDefinition {
        id: 50,
        title: "雇用統計",
        category: CalendarEventCategory::Indicator,
        hour: 8,
        minute: 30,
    },
    ReleaseDefinition {
        id: 54,
        title: "PCE 物価指数",
        category: CalendarEventCategory::Indicator,
        hour: 8,
        minute: 30,
    },
    ReleaseDefinition {
        id: 53,
        title: "国内総生産 (GDP)",
        category: CalendarEventCategory::Indicator,
        hour: 8,
        minute: 30,
    },
    ReleaseDefinition {
        id: 9,
        title: "小売売上高",
        category: CalendarEventCategory::Indicator,
        hour: 8,
        minute: 30,
    },
    ReleaseDefinition {
        id: 101,
        title: "FOMC 声明",
        category: CalendarEventCategory::CentralBank,
        hour: 14,
        minute: 0,
    },
];

#[derive(Debug, Deserialize)]
struct ReleaseDatesResponse {
    count: usize,
    release_dates: Vec<RawReleaseDate>,
}

#[derive(Debug, Deserialize)]
struct RawReleaseDate {
    release_id: i32,
    date: String,
}

impl FredClient {
    fn build_release_dates_url(
        &self,
        release_id: i32,
        offset: usize,
    ) -> Result<Url, CalendarEventSourceError> {
        let mut url = Url::parse(&self.release_dates_base_url).map_err(|error| {
            CalendarEventSourceError::Failed(format!("invalid release dates base URL: {error}"))
        })?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("release_id", &release_id.to_string());
            query.append_pair("api_key", &self.api_key);
            query.append_pair("file_type", "json");
            query.append_pair("limit", &RELEASE_DATES_LIMIT.to_string());
            query.append_pair("offset", &offset.to_string());
            query.append_pair("sort_order", "asc");
            query.append_pair("include_release_dates_with_no_data", "true");
        }
        Ok(url)
    }

    async fn fetch_release_dates(
        &self,
        release_id: i32,
    ) -> Result<Vec<NaiveDate>, CalendarEventSourceError> {
        let mut dates = Vec::new();
        let mut offset = 0;
        loop {
            let url = self.build_release_dates_url(release_id, offset)?;
            let response = self.http.get(url).send().await.map_err(|error| {
                CalendarEventSourceError::Failed(error.without_url().to_string())
            })?;
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                return Err(CalendarEventSourceError::Failed(format!(
                    "FRED returned status {status} for release {release_id}"
                )));
            }

            let body = response.text().await.map_err(|error| {
                CalendarEventSourceError::Failed(error.without_url().to_string())
            })?;
            let page = parse_release_dates(&body, release_id)?;
            let count = page.count;
            let received = page.dates.len();
            let received_end = offset.checked_add(received).ok_or_else(|| {
                CalendarEventSourceError::Failed("release dates offset overflowed".to_string())
            })?;
            dates.extend(page.dates);

            if received_end >= count {
                break;
            }
            if received == 0 {
                return Err(CalendarEventSourceError::Failed(format!(
                    "FRED returned an incomplete page for release {release_id}"
                )));
            }
            offset = received_end;
        }
        Ok(dates)
    }
}

#[async_trait]
impl CalendarEventSource for FredClient {
    fn source(&self) -> &str {
        "fred"
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let to = today
            .checked_add_months(Months::new(LOOKAHEAD_MONTHS))
            .ok_or_else(|| {
                CalendarEventSourceError::Failed("calendar date range overflowed".to_string())
            })?;
        let date_range = DateRange { from: today, to };
        let mut events = Vec::new();

        for release in RELEASES {
            for release_date in self.fetch_release_dates(release.id).await? {
                let event = calendar_event(release, release_date)?;
                if (date_range.from..=date_range.to).contains(&event.event_date) {
                    events.push(event);
                }
            }
        }

        events.sort_by(|left, right| {
            (&left.event_date, &left.event_at, &left.external_id).cmp(&(
                &right.event_date,
                &right.event_at,
                &right.external_id,
            ))
        });
        Ok(CalendarEventBatch { date_range, events })
    }
}

fn parse_release_dates(
    body: &str,
    expected_release_id: i32,
) -> Result<ParsedReleaseDates, CalendarEventSourceError> {
    let parsed: ReleaseDatesResponse = serde_json::from_str(body).map_err(|error| {
        CalendarEventSourceError::Failed(format!("FRED release dates response: {error}"))
    })?;
    let mut dates = Vec::with_capacity(parsed.release_dates.len());
    for release_date in parsed.release_dates {
        if release_date.release_id != expected_release_id {
            return Err(CalendarEventSourceError::Failed(format!(
                "FRED returned release {} while requesting release {expected_release_id}",
                release_date.release_id
            )));
        }
        dates.push(
            NaiveDate::parse_from_str(&release_date.date, "%Y-%m-%d").map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "invalid release date '{}': {error}",
                    release_date.date
                ))
            })?,
        );
    }
    Ok(ParsedReleaseDates {
        count: parsed.count,
        dates,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedReleaseDates {
    count: usize,
    dates: Vec<NaiveDate>,
}

fn calendar_event(
    release: ReleaseDefinition,
    release_date: NaiveDate,
) -> Result<CalendarEvent, CalendarEventSourceError> {
    let local_datetime = release_date
        .and_hms_opt(release.hour, release.minute, 0)
        .ok_or_else(|| CalendarEventSourceError::Failed("invalid release time".to_string()))?;
    let eastern_datetime = New_York
        .from_local_datetime(&local_datetime)
        .single()
        .ok_or_else(|| {
            CalendarEventSourceError::Failed(format!(
                "could not resolve Eastern Time for release {} on {release_date}",
                release.id
            ))
        })?;
    let event_at = eastern_datetime.with_timezone(&Utc);
    let event_date = event_at.with_timezone(&Tokyo).date_naive();

    Ok(CalendarEvent {
        source: "fred".to_string(),
        external_id: format!("{}:{release_date}", release.id),
        category: release.category,
        country: "US".to_string(),
        title: release.title.to_string(),
        stock_id: None,
        fiscal_period: None,
        event_date,
        event_at: Some(event_at),
        time_of_day: None,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use core_application::calendar::source::CalendarEventSourceError;
    use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory};
    use indoc::indoc;
    use rstest::rstest;

    use crate::FredClient;

    use super::{ParsedReleaseDates, ReleaseDefinition, calendar_event, parse_release_dates};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &date(year, month, day)
                .and_hms_opt(hour, minute, 0)
                .expect("time"),
        )
    }

    #[rstest]
    fn test_parse_release_dates() {
        let body = indoc! {r#"
            {
                "count": 2,
                "release_dates": [
                    {"release_id": 9001, "date": "2040-01-15"},
                    {"release_id": 9001, "date": "2040-02-15"}
                ]
            }
        "#};

        assert_eq!(
            parse_release_dates(body, 9001),
            Ok(ParsedReleaseDates {
                count: 2,
                dates: vec![date(2040, 1, 15), date(2040, 2, 15)],
            }),
        );
    }

    #[rstest]
    #[case::winter_indicator(
        ReleaseDefinition {
            id: 9001,
            title: "サンプル指標",
            category: CalendarEventCategory::Indicator,
            hour: 8,
            minute: 30,
        },
        date(2040, 1, 15),
        date(2040, 1, 15),
        utc(2040, 1, 15, 13, 30),
    )]
    #[case::summer_indicator(
        ReleaseDefinition {
            id: 9001,
            title: "サンプル指標",
            category: CalendarEventCategory::Indicator,
            hour: 8,
            minute: 30,
        },
        date(2040, 7, 15),
        date(2040, 7, 15),
        utc(2040, 7, 15, 12, 30),
    )]
    #[case::winter_afternoon_release_next_day_in_tokyo(
        ReleaseDefinition {
            id: 9002,
            title: "サンプル中銀発表",
            category: CalendarEventCategory::CentralBank,
            hour: 14,
            minute: 0,
        },
        date(2040, 1, 15),
        date(2040, 1, 16),
        utc(2040, 1, 15, 19, 0),
    )]
    #[case::summer_afternoon_release_next_day_in_tokyo(
        ReleaseDefinition {
            id: 9002,
            title: "サンプル中銀発表",
            category: CalendarEventCategory::CentralBank,
            hour: 14,
            minute: 0,
        },
        date(2040, 7, 15),
        date(2040, 7, 16),
        utc(2040, 7, 15, 18, 0),
    )]
    fn test_calendar_event_converts_eastern_time_to_tokyo(
        #[case] release: ReleaseDefinition,
        #[case] release_date: NaiveDate,
        #[case] expected_event_date: NaiveDate,
        #[case] expected_event_at: DateTime<Utc>,
    ) {
        assert_eq!(
            calendar_event(release, release_date),
            Ok(CalendarEvent {
                source: "fred".to_string(),
                external_id: format!("{}:{release_date}", release.id),
                category: release.category,
                country: "US".to_string(),
                title: release.title.to_string(),
                stock_id: None,
                fiscal_period: None,
                event_date: expected_event_date,
                event_at: Some(expected_event_at),
                time_of_day: None,
            }),
        );
    }

    #[rstest]
    fn test_build_release_dates_url_includes_future_releases_and_pagination() {
        let client = FredClient::with_base_urls(
            "sample-key".to_string(),
            "https://api.example.test/observations",
            "https://api.example.test/release/dates",
        )
        .expect("client");

        let url = client
            .build_release_dates_url(9001, 20)
            .expect("release dates URL");

        assert_eq!(
            url.as_str(),
            "https://api.example.test/release/dates?release_id=9001&api_key=sample-key&file_type=json&limit=10000&offset=20&sort_order=asc&include_release_dates_with_no_data=true"
        );
    }

    #[rstest]
    #[case::wrong_release_id(
        indoc! {r#"{"count": 1, "release_dates": [{"release_id": 9002, "date": "2040-01-15"}]}"#},
        CalendarEventSourceError::Failed(
            "FRED returned release 9002 while requesting release 9001".to_string(),
        ),
    )]
    #[case::invalid_date(
        indoc! {r#"{"count": 1, "release_dates": [{"release_id": 9001, "date": "not-a-date"}]}"#},
        CalendarEventSourceError::Failed(
            "invalid release date 'not-a-date': input contains invalid characters".to_string(),
        ),
    )]
    fn test_parse_release_dates_rejects_inconsistent_data(
        #[case] body: &str,
        #[case] expected: CalendarEventSourceError,
    ) {
        assert_eq!(parse_release_dates(body, 9001), Err(expected));
    }
}
