//! Alpha Vantage の EARNINGS_CALENDAR から決算予定を取得する。

use std::time::Duration;

use async_trait::async_trait;
use chrono::{Days, Months, NaiveDate};
use core_application::{
    calendar::source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
    daily_bar_source::DateRange,
};
use core_domain::calendar_event::{CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay};
use reqwest::Url;
use serde::Deserialize;

const DEFAULT_BASE_URL: &str = "https://www.alphavantage.co/query";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const HORIZON_MONTHS: u32 = 3;
const SOURCE: &str = "alpha_vantage";

pub struct AlphaVantageClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl AlphaVantageClient {
    pub fn new(api_key: String) -> Result<Self, CalendarEventSourceError> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(
        api_key: String,
        base_url: &str,
    ) -> Result<Self, CalendarEventSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| {
                CalendarEventSourceError::Failed(format!(
                    "failed to initialize Alpha Vantage client: {error}"
                ))
            })?;
        Ok(Self {
            http,
            base_url: base_url.to_string(),
            api_key,
        })
    }

    fn build_url(&self) -> Result<Url, CalendarEventSourceError> {
        let mut url = Url::parse(&self.base_url).map_err(|error| {
            CalendarEventSourceError::Failed(format!("invalid Alpha Vantage base URL: {error}"))
        })?;
        url.query_pairs_mut()
            .append_pair("function", "EARNINGS_CALENDAR")
            .append_pair("horizon", &format!("{HORIZON_MONTHS}month"))
            .append_pair("apikey", &self.api_key);
        Ok(url)
    }
}

#[async_trait]
impl CalendarEventSource for AlphaVantageClient {
    fn source(&self) -> &str {
        SOURCE
    }

    async fn fetch_calendar_events(
        &self,
        today: NaiveDate,
    ) -> Result<CalendarEventBatch, CalendarEventSourceError> {
        let url = self.build_url()?;
        let response = self.http.get(url).send().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "Alpha Vantage request failed: {}",
                error.without_url()
            ))
        })?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(CalendarEventSourceError::Failed(format!(
                "Alpha Vantage returned HTTP status {status}"
            )));
        }

        let body = response.text().await.map_err(|error| {
            CalendarEventSourceError::Failed(format!(
                "failed to read Alpha Vantage response: {}",
                error.without_url()
            ))
        })?;
        parse_earnings_calendar(&body, today)
    }
}

#[derive(Debug, Deserialize)]
struct RawEarningsCalendarRow {
    symbol: String,
    name: String,
    #[serde(rename = "reportDate")]
    report_date: String,
    #[serde(rename = "fiscalDateEnding")]
    fiscal_date_ending: String,
    #[serde(rename = "timeOfTheDay")]
    time_of_the_day: String,
}

fn parse_earnings_calendar(
    body: &str,
    today: NaiveDate,
) -> Result<CalendarEventBatch, CalendarEventSourceError> {
    let end_date = today
        .checked_add_months(Months::new(HORIZON_MONTHS))
        .and_then(|date| date.checked_add_days(Days::new(1)))
        .ok_or_else(|| {
            CalendarEventSourceError::Failed("Alpha Vantage date range overflowed".to_string())
        })?;
    let mut reader = csv::Reader::from_reader(body.as_bytes());
    let headers = reader.headers().map_err(|error| {
        CalendarEventSourceError::Failed(format!("invalid Alpha Vantage CSV header: {error}"))
    })?;
    for required in [
        "symbol",
        "name",
        "reportDate",
        "fiscalDateEnding",
        "timeOfTheDay",
    ] {
        if !headers.iter().any(|header| header == required) {
            return Err(CalendarEventSourceError::Failed(format!(
                "Alpha Vantage CSV is missing the {required} column"
            )));
        }
    }

    let mut events = Vec::new();
    for row in reader.deserialize::<RawEarningsCalendarRow>() {
        let row = row.map_err(|error| {
            CalendarEventSourceError::Failed(format!("invalid Alpha Vantage CSV row: {error}"))
        })?;
        let symbol = row.symbol.trim();
        let name = row.name.trim();
        if symbol.is_empty() || name.is_empty() {
            return Err(CalendarEventSourceError::Failed(
                "Alpha Vantage CSV contains an empty symbol or name".to_string(),
            ));
        }

        let report_date = parse_date(&row.report_date, "reportDate")?;
        let fiscal_period = parse_date(&row.fiscal_date_ending, "fiscalDateEnding")?;
        let time_of_day = parse_time_of_day(&row.time_of_the_day);
        let event_date = match time_of_day {
            // 米国市場の引け後は JST では翌日の早朝です。
            Some(CalendarEventTimeOfDay::PostMarket) => report_date.checked_add_days(Days::new(1)),
            _ => Some(report_date),
        }
        .ok_or_else(|| {
            CalendarEventSourceError::Failed("Alpha Vantage event date overflowed".to_string())
        })?;
        let stock_id = format!("US:{}", symbol.replace('.', "-"));

        events.push(CalendarEvent {
            source: SOURCE.to_string(),
            external_id: format!("{stock_id}:{fiscal_period}"),
            category: CalendarEventCategory::Earnings,
            country: "US".to_string(),
            title: name.to_string(),
            stock_id: Some(stock_id),
            fiscal_period: Some(fiscal_period.format("%Y-%m-%d").to_string()),
            event_date,
            event_at: None,
            time_of_day,
        });
    }

    Ok(CalendarEventBatch {
        date_range: DateRange {
            from: today,
            to: end_date,
        },
        events,
    })
}

fn parse_date(value: &str, field: &str) -> Result<NaiveDate, CalendarEventSourceError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| {
        CalendarEventSourceError::Failed(format!(
            "invalid Alpha Vantage {field} value '{value}': {error}"
        ))
    })
}

fn parse_time_of_day(value: &str) -> Option<CalendarEventTimeOfDay> {
    if value.trim().eq_ignore_ascii_case("pre-market") {
        Some(CalendarEventTimeOfDay::PreMarket)
    } else if value.trim().eq_ignore_ascii_case("post-market") {
        Some(CalendarEventTimeOfDay::PostMarket)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::{
        calendar::source::{CalendarEventBatch, CalendarEventSource, CalendarEventSourceError},
        daily_bar_source::DateRange,
    };
    use core_domain::calendar_event::{
        CalendarEvent, CalendarEventCategory, CalendarEventTimeOfDay,
    };
    use indoc::indoc;
    use rstest::rstest;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{AlphaVantageClient, SOURCE, parse_earnings_calendar};

    const FIXTURE: &str = include_str!("../tests/fixtures/earnings_calendar.csv");

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn expected_batch() -> CalendarEventBatch {
        CalendarEventBatch {
            date_range: DateRange {
                from: date(2026, 10, 4),
                to: date(2027, 1, 5),
            },
            events: vec![
                CalendarEvent {
                    source: SOURCE.to_string(),
                    external_id: "US:QZX:2026-09-30".to_string(),
                    category: CalendarEventCategory::Earnings,
                    country: "US".to_string(),
                    title: "Example Holdings".to_string(),
                    stock_id: Some("US:QZX".to_string()),
                    fiscal_period: Some("2026-09-30".to_string()),
                    event_date: date(2026, 10, 8),
                    event_at: None,
                    time_of_day: Some(CalendarEventTimeOfDay::PreMarket),
                },
                CalendarEvent {
                    source: SOURCE.to_string(),
                    external_id: "US:XYZ-A:2026-09-30".to_string(),
                    category: CalendarEventCategory::Earnings,
                    country: "US".to_string(),
                    title: "Sample Industries".to_string(),
                    stock_id: Some("US:XYZ-A".to_string()),
                    fiscal_period: Some("2026-09-30".to_string()),
                    event_date: date(2026, 10, 9),
                    event_at: None,
                    time_of_day: Some(CalendarEventTimeOfDay::PostMarket),
                },
                CalendarEvent {
                    source: SOURCE.to_string(),
                    external_id: "US:LMN:2026-09-30".to_string(),
                    category: CalendarEventCategory::Earnings,
                    country: "US".to_string(),
                    title: "Demo Group".to_string(),
                    stock_id: Some("US:LMN".to_string()),
                    fiscal_period: Some("2026-09-30".to_string()),
                    event_date: date(2026, 10, 10),
                    event_at: None,
                    time_of_day: None,
                },
            ],
        }
    }

    #[test]
    fn parses_calendar_fixture_and_normalizes_post_market_dates() {
        assert_eq!(
            parse_earnings_calendar(FIXTURE, date(2026, 10, 4)),
            Ok(expected_batch()),
        );
    }

    #[tokio::test]
    async fn fetches_and_parses_the_calendar_fixture() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("function", "EARNINGS_CALENDAR"))
            .and(query_param("horizon", "3month"))
            .and(query_param("apikey", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string(FIXTURE))
            .expect(1)
            .mount(&server)
            .await;
        let client =
            AlphaVantageClient::with_base_url("test-key".into(), &server.uri()).expect("client");

        assert_eq!(
            client.fetch_calendar_events(date(2026, 10, 4)).await,
            Ok(expected_batch()),
        );
    }

    #[tokio::test]
    async fn rejects_non_successful_http_responses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
        let client =
            AlphaVantageClient::with_base_url("test-key".into(), &server.uri()).expect("client");

        let result = client
            .fetch_calendar_events(date(2026, 10, 4))
            .await
            .map_err(|error| match error {
                CalendarEventSourceError::Failed(message) => message,
            });

        assert_eq!(result, Err("Alpha Vantage returned HTTP status 503".into()));
    }

    #[rstest]
    #[case::missing_required_column(
        indoc! {"
            symbol,name,reportDate,fiscalDateEnding
            QZX,Example Holdings,2026-10-08,2026-09-30
        "},
        "Alpha Vantage CSV is missing the timeOfTheDay column"
    )]
    fn rejects_invalid_calendar_csv(#[case] body: &str, #[case] expected: &str) {
        assert_eq!(
            parse_earnings_calendar(body, date(2026, 10, 4)).map_err(|error| match error {
                CalendarEventSourceError::Failed(message) => message,
            }),
            Err(expected.to_string()),
        );
    }
}
