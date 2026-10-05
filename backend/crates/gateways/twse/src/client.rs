#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, str::FromStr, time::Duration};

use async_trait::async_trait;
use chrono::{Datelike, Duration as ChronoDuration, NaiveDate, Weekday};
use core_application::{
    indicator_observation::IndicatorObservationBatchError,
    indicator_observation_batch_source::{
        IndicatorObservationBatch, IndicatorObservationBatchSource,
    },
    indicator_observation_source::IndicatorObservationSourceError,
};
use core_domain::IndicatorObservation;
use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Url, header::RETRY_AFTER};
use rust_decimal::Decimal;
use serde::Deserialize;

const DEFAULT_BASE_URL: &str = "https://www.twse.com.tw/exchangeReport/MI_INDEX";
const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_HOST_KEY: &str = "twse:host:www.twse.com.tw";
const RATE_LIMIT_LIMIT: u32 = 1;
const RATE_LIMIT_PERIOD: Duration = Duration::from_secs(1);
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(60);
const RATE_LIMIT_FALLBACK_COOLDOWN: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX_COOLDOWN: Duration = Duration::from_secs(60 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

const INDEX_NAMES: [(&str, &str); 2] =
    [("發行量加權股價指數", "TAIEX"), ("半導體類指數", "TW_SEMI")];

#[derive(Deserialize)]
struct IndexResponse {
    stat: String,
    tables: Option<Vec<IndexTable>>,
}

#[derive(Deserialize)]
struct IndexTable {
    data: Option<Vec<Vec<String>>>,
}

enum TwseRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    TestNoop,
    #[cfg(test)]
    TestRecorder(Arc<Mutex<Vec<Duration>>>),
}

impl TwseRateLimiter {
    async fn acquire(&self) -> Result<(), IndicatorObservationSourceError> {
        match self {
            Self::Shared(rate_limiter) => rate_limiter
                .acquire(
                    &[Quota::new(
                        RATE_LIMIT_HOST_KEY,
                        RATE_LIMIT_LIMIT,
                        RATE_LIMIT_PERIOD,
                    )],
                    RATE_LIMIT_MAX_WAIT,
                )
                .await
                .map_err(map_rate_limit_error),
            #[cfg(test)]
            Self::TestNoop | Self::TestRecorder(_) => Ok(()),
        }
    }

    async fn penalize(&self, duration: Duration) -> Result<(), IndicatorObservationSourceError> {
        match self {
            Self::Shared(rate_limiter) => rate_limiter
                .penalize(RATE_LIMIT_HOST_KEY, duration)
                .await
                .map_err(map_rate_limit_error),
            #[cfg(test)]
            Self::TestNoop => Ok(()),
            #[cfg(test)]
            Self::TestRecorder(penalties) => {
                penalties
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(duration);
                Ok(())
            }
        }
    }
}

pub struct TwseClient {
    http: reqwest::Client,
    base_url: String,
    rate_limiter: TwseRateLimiter,
}

impl TwseClient {
    pub fn new(redis_url: &str) -> Result<Self, IndicatorObservationSourceError> {
        let rate_limiter =
            RateLimiter::new(redis_url, RATE_LIMIT_KEY_PREFIX).map_err(map_rate_limit_error)?;
        Self::with_rate_limiter(DEFAULT_BASE_URL, TwseRateLimiter::Shared(rate_limiter))
    }

    fn with_rate_limiter(
        base_url: &str,
        rate_limiter: TwseRateLimiter,
    ) -> Result<Self, IndicatorObservationSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| IndicatorObservationSourceError::Initialization(error.to_string()))?;

        Ok(Self {
            http,
            base_url: base_url.to_string(),
            rate_limiter,
        })
    }

    #[cfg(test)]
    fn without_rate_limiter(base_url: &str) -> Result<Self, IndicatorObservationSourceError> {
        Self::with_rate_limiter(base_url, TwseRateLimiter::TestNoop)
    }

    #[cfg(test)]
    fn with_recording_rate_limiter(
        base_url: &str,
        penalties: Arc<Mutex<Vec<Duration>>>,
    ) -> Result<Self, IndicatorObservationSourceError> {
        Self::with_rate_limiter(base_url, TwseRateLimiter::TestRecorder(penalties))
    }

    fn build_url(&self, date: NaiveDate) -> Result<Url, IndicatorObservationSourceError> {
        let mut url = Url::parse(&self.base_url).map_err(|error| {
            IndicatorObservationSourceError::Parse(format!("invalid TWSE base URL: {error}"))
        })?;
        url.query_pairs_mut()
            .append_pair("response", "json")
            .append_pair("date", &date.format("%Y%m%d").to_string())
            .append_pair("type", "IND");
        Ok(url)
    }

    async fn fetch_date(
        &self,
        date: NaiveDate,
    ) -> Result<HashMap<String, Decimal>, IndicatorObservationSourceError> {
        self.rate_limiter.acquire().await?;
        let response = self
            .http
            .get(self.build_url(date)?)
            .send()
            .await
            .map_err(|error| {
                IndicatorObservationSourceError::Network(error.without_url().to_string())
            })?;

        let status = response.status().as_u16();
        if matches!(status, 429 | 503) {
            let cooldown = retry_after_duration(
                response
                    .headers()
                    .get(RETRY_AFTER)
                    .and_then(|value| value.to_str().ok()),
            );
            self.rate_limiter.penalize(cooldown).await?;
        }
        if !(200..300).contains(&status) {
            return Err(IndicatorObservationSourceError::Api {
                status,
                message: format!("TWSE returned status {status} for {date}"),
            });
        }

        let body = response.text().await.map_err(|error| {
            IndicatorObservationSourceError::Network(error.without_url().to_string())
        })?;
        parse_index_values(&body)
    }
}

#[async_trait]
impl IndicatorObservationBatchSource for TwseClient {
    async fn fetch_observations(
        &self,
        series_ids: &[&str],
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<IndicatorObservationBatch, IndicatorObservationSourceError> {
        if from > to {
            return Err(IndicatorObservationSourceError::Parse(
                "observation start date must be on or before end date".to_string(),
            ));
        }

        for series_id in series_ids {
            if !INDEX_NAMES.iter().any(|(_, id)| id == series_id) {
                return Err(IndicatorObservationSourceError::Parse(format!(
                    "unsupported TWSE series: {series_id}"
                )));
            }
        }

        let mut observations: HashMap<_, Vec<_>> = series_ids
            .iter()
            .map(|series_id| ((*series_id).to_string(), Vec::new()))
            .collect();
        let mut errors = Vec::new();
        let mut date = from;
        loop {
            if !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
                match self.fetch_date(date).await {
                    Ok(values) => {
                        for (series_id, value) in values {
                            if let Some(series) = observations.get_mut(&series_id) {
                                series.push(IndicatorObservation { date, value });
                            }
                        }
                    }
                    Err(error) => errors.push(IndicatorObservationBatchError {
                        date,
                        message: error.to_string(),
                    }),
                }
            }
            if date == to {
                break;
            }
            date = date
                .checked_add_signed(ChronoDuration::days(1))
                .ok_or_else(|| {
                    IndicatorObservationSourceError::Parse(
                        "observation date range exceeds the supported range".to_string(),
                    )
                })?;
        }
        Ok(IndicatorObservationBatch {
            observations,
            errors,
        })
    }
}

fn parse_index_values(
    body: &str,
) -> Result<HashMap<String, Decimal>, IndicatorObservationSourceError> {
    let response: IndexResponse = serde_json::from_str(body).map_err(|error| {
        IndicatorObservationSourceError::Parse(format!("TWSE response: {error}"))
    })?;
    if response.stat != "OK" {
        return Err(IndicatorObservationSourceError::Parse(format!(
            "TWSE response status: {}",
            response.stat
        )));
    }

    let tables = response.tables.ok_or_else(|| {
        IndicatorObservationSourceError::Parse(
            "TWSE response is missing the index tables".to_string(),
        )
    })?;
    let mut values = HashMap::new();
    for row in tables
        .iter()
        .filter_map(|table| table.data.as_ref())
        .flatten()
    {
        let Some((name, raw_value)) = row.first().zip(row.get(1)) else {
            continue;
        };
        let Some((_, series_id)) = INDEX_NAMES
            .iter()
            .find(|(index_name, _)| *index_name == name.as_str())
        else {
            continue;
        };
        let value = Decimal::from_str(&raw_value.replace(',', "")).map_err(|error| {
            IndicatorObservationSourceError::Parse(format!(
                "invalid TWSE value for {series_id}: {error}"
            ))
        })?;
        values.insert((*series_id).to_string(), value);
    }
    if !values.is_empty() && values.len() != INDEX_NAMES.len() {
        return Err(IndicatorObservationSourceError::Parse(
            "TWSE response contains an incomplete set of index rows".to_string(),
        ));
    }
    Ok(values)
}

fn retry_after_duration(value: Option<&str>) -> Duration {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds).min(RATE_LIMIT_MAX_COOLDOWN))
        .unwrap_or(RATE_LIMIT_FALLBACK_COOLDOWN)
}

fn map_rate_limit_error(error: RateLimitError) -> IndicatorObservationSourceError {
    IndicatorObservationSourceError::Network(format!("TWSE rate limit: {error}"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::error::Error;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use chrono::NaiveDate;
    use core_domain::IndicatorObservation;
    use indoc::indoc;
    use rate_limit::{Quota, RateLimitError, RateLimiter};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        RATE_LIMIT_HOST_KEY, RATE_LIMIT_PERIOD, TwseClient, TwseRateLimiter, parse_index_values,
        retry_after_duration,
    };
    use core_application::indicator_observation::IndicatorObservationBatchError;
    use core_application::indicator_observation_batch_source::IndicatorObservationBatch;
    use core_application::indicator_observation_batch_source::IndicatorObservationBatchSource;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[test]
    fn parse_index_values_maps_both_indices_and_removes_thousands_separators() {
        let body = indoc! {r#"
            {
              "stat": "OK",
              "tables": [
                {
                  "data": [
                    ["發行量加權股價指數", "12,345.67", "", "", "", ""],
                    ["半導體類指數", "8,765.43", "", "", "", ""]
                  ]
                },
                {"data": null}
              ]
            }
        "#};

        assert_eq!(
            parse_index_values(body).expect("parse response"),
            HashMap::from([
                (
                    "TAIEX".to_string(),
                    Decimal::from_str_exact("12345.67").expect("valid decimal"),
                ),
                (
                    "TW_SEMI".to_string(),
                    Decimal::from_str_exact("8765.43").expect("valid decimal"),
                ),
            ]),
        );
    }

    #[test]
    fn parse_index_values_accepts_a_valid_response_without_trading_rows() {
        let body = r#"{"stat":"OK","date":"example","tables":[{"title":"example index table","data":[]},{"data":null}]}"#;

        assert_eq!(
            parse_index_values(body).expect("parse response"),
            HashMap::new()
        );
    }

    #[rstest]
    #[case::unexpected_status(
        r#"{"stat":"ERROR","tables":[]}"#,
        "failed to parse response: TWSE response status: ERROR"
    )]
    #[case::missing_tables(
        r#"{"stat":"OK"}"#,
        "failed to parse response: TWSE response is missing the index tables"
    )]
    #[case::partial_index_rows(
        r#"{"stat":"OK","tables":[{"data":[["發行量加權股價指數","12.34"]]}]}"#,
        "failed to parse response: TWSE response contains an incomplete set of index rows"
    )]
    fn parse_index_values_reports_invalid_response_contracts(
        #[case] body: &str,
        #[case] expected_error: &str,
    ) {
        let result = parse_index_values(body)
            .map(|_| ())
            .map_err(|error| error.to_string());

        assert_eq!(result, Err(expected_error.to_string()));
    }

    #[test]
    fn parse_index_values_reports_invalid_json() {
        let body = "{";
        let json_error = serde_json::from_str::<serde_json::Value>(body)
            .expect_err("invalid JSON")
            .to_string();
        let result = parse_index_values(body)
            .map(|_| ())
            .map_err(|error| error.to_string());

        assert_eq!(
            result,
            Err(format!(
                "failed to parse response: TWSE response: {json_error}"
            )),
        );
    }

    #[test]
    fn parse_index_values_reports_invalid_index_value() {
        let body = r#"{"stat":"OK","tables":[{"data":[["發行量加權股價指數","invalid"]]}]}"#;
        let decimal_error = Decimal::from_str("invalid")
            .expect_err("invalid decimal")
            .to_string();
        let result = parse_index_values(body)
            .map(|_| ())
            .map_err(|error| error.to_string());

        assert_eq!(
            result,
            Err(format!(
                "failed to parse response: invalid TWSE value for TAIEX: {decimal_error}"
            )),
        );
    }

    #[rstest]
    #[case::retry_after("30", Duration::from_secs(30))]
    #[case::missing_header("invalid", Duration::from_secs(60))]
    #[case::maximum_cooldown("999999", Duration::from_secs(60 * 60))]
    fn retry_after_uses_a_bounded_cooldown(#[case] value: &str, #[case] expected: Duration) {
        assert_eq!(retry_after_duration(Some(value)), expected);
    }

    #[rstest]
    #[case::too_many_requests(429)]
    #[case::service_unavailable(503)]
    #[case::internal_server_error(500)]
    #[tokio::test]
    async fn fetch_observations_returns_api_error_for_http_failures(#[case] status: u16) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/exchangeReport/MI_INDEX"))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("Retry-After", "5")
                    .set_body_string("{}"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let penalties = Arc::new(Mutex::new(Vec::new()));
        let client = TwseClient::with_recording_rate_limiter(
            &format!("{}/exchangeReport/MI_INDEX", server.uri()),
            penalties.clone(),
        )
        .expect("client");
        let result = client
            .fetch_observations(&["TAIEX"], date(2099, 1, 2), date(2099, 1, 2))
            .await
            .map(|batch| batch.errors)
            .map_err(|error| error.to_string());

        let penalties = penalties
            .lock()
            .expect("lock rate limiter penalties")
            .clone();
        let expected_penalties = if matches!(status, 429 | 503) {
            vec![Duration::from_secs(5)]
        } else {
            Vec::new()
        };

        assert_eq!(
            (result, penalties),
            (
                Ok(vec![IndicatorObservationBatchError {
                    date: date(2099, 1, 2),
                    message: format!(
                        "api error (status {status}): TWSE returned status {status} for 2099-01-02"
                    ),
                }]),
                expected_penalties,
            ),
        );
    }

    #[tokio::test]
    async fn fetch_observations_keeps_later_dates_when_a_date_fails() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/exchangeReport/MI_INDEX"))
            .and(query_param("date", "20990102"))
            .respond_with(
                ResponseTemplate::new(503)
                    .insert_header("Retry-After", "5")
                    .set_body_string("{}"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/exchangeReport/MI_INDEX"))
            .and(query_param("date", "20990105"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {"stat":"OK","tables":[{"data":[
                  ["發行量加權股價指數","123.45"],
                  ["半導體類指數","67.89"]
                ]}]}
            "#}))
            .expect(1)
            .mount(&server)
            .await;

        let penalties = Arc::new(Mutex::new(Vec::new()));
        let client = TwseClient::with_recording_rate_limiter(
            &format!("{}/exchangeReport/MI_INDEX", server.uri()),
            penalties.clone(),
        )
        .expect("client");
        let result = client
            .fetch_observations(&["TAIEX", "TW_SEMI"], date(2099, 1, 2), date(2099, 1, 5))
            .await
            .map_err(|error| error.to_string());
        let penalties = penalties
            .lock()
            .expect("lock rate limiter penalties")
            .clone();

        assert_eq!(
            (result, penalties),
            (
                Ok(IndicatorObservationBatch {
                    observations: HashMap::from([
                        (
                            "TAIEX".to_string(),
                            vec![IndicatorObservation {
                                date: date(2099, 1, 5),
                                value: Decimal::from_str_exact("123.45").expect("valid decimal"),
                            }],
                        ),
                        (
                            "TW_SEMI".to_string(),
                            vec![IndicatorObservation {
                                date: date(2099, 1, 5),
                                value: Decimal::from_str_exact("67.89").expect("valid decimal"),
                            }],
                        ),
                    ]),
                    errors: vec![IndicatorObservationBatchError {
                        date: date(2099, 1, 2),
                        message: "api error (status 503): TWSE returned status 503 for 2099-01-02"
                            .to_string(),
                    }],
                }),
                vec![Duration::from_secs(5)],
            ),
        );
    }

    #[tokio::test]
    async fn a_429_penalizes_other_twse_clients_sharing_the_redis_prefix()
    -> Result<(), Box<dyn Error>> {
        let redis_url = std::env::var("REDIS_URL")?;
        let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let key_prefix = format!("t-rader:test:twse:{unique_suffix}:");
        let client_limiter = RateLimiter::new(&redis_url, &key_prefix)?;
        let other_client_limiter = RateLimiter::new(&redis_url, &key_prefix)?;
        let redis_ready = client_limiter
            .acquire(
                &[Quota::new("preflight", 1, Duration::from_secs(1))],
                Duration::ZERO,
            )
            .await;
        if matches!(&redis_ready, Err(RateLimitError::RedisUnavailable)) {
            return Ok(());
        }
        redis_ready?;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "5")
                    .set_body_string("{}"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = TwseClient::with_rate_limiter(
            &format!("{}/exchangeReport/MI_INDEX", server.uri()),
            TwseRateLimiter::Shared(client_limiter),
        )?;

        let errors = client
            .fetch_observations(&["TAIEX"], date(2099, 1, 2), date(2099, 1, 2))
            .await
            .map(|batch| batch.errors)
            .map_err(|error| error.to_string());
        let blocked_result = other_client_limiter
            .acquire(
                &[Quota::new(RATE_LIMIT_HOST_KEY, 1, RATE_LIMIT_PERIOD)],
                Duration::ZERO,
            )
            .await;

        assert_eq!(
            (errors, blocked_result),
            (
                Ok(vec![IndicatorObservationBatchError {
                    date: date(2099, 1, 2),
                    message: "api error (status 429): TWSE returned status 429 for 2099-01-02"
                        .to_string(),
                }]),
                Err(RateLimitError::MaxWaitExceeded {
                    max_wait: Duration::ZERO,
                }),
            ),
        );
        Ok(())
    }

    #[tokio::test]
    async fn fetch_observations_requests_each_weekday_once_and_skips_weekends() {
        let server = MockServer::start().await;
        for query_date in ["20990102", "20990105"] {
            Mock::given(method("GET"))
                .and(path("/exchangeReport/MI_INDEX"))
                .and(query_param("response", "json"))
                .and(query_param("date", query_date))
                .and(query_param("type", "IND"))
                .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                    {"stat":"OK","tables":[{"data":[
                      ["發行量加權股價指數","123.45"],
                      ["半導體類指數","67.89"]
                    ]}]}
                "#}))
                .expect(1)
                .mount(&server)
                .await;
        }

        let client =
            TwseClient::without_rate_limiter(&format!("{}/exchangeReport/MI_INDEX", server.uri()))
                .expect("client");
        let observations = client
            .fetch_observations(&["TAIEX", "TW_SEMI"], date(2099, 1, 2), date(2099, 1, 5))
            .await
            .expect("fetch observations");

        assert_eq!(
            observations,
            IndicatorObservationBatch {
                observations: HashMap::from([
                    (
                        "TAIEX".to_string(),
                        vec![
                            IndicatorObservation {
                                date: date(2099, 1, 2),
                                value: Decimal::from_str_exact("123.45").expect("valid decimal"),
                            },
                            IndicatorObservation {
                                date: date(2099, 1, 5),
                                value: Decimal::from_str_exact("123.45").expect("valid decimal"),
                            },
                        ],
                    ),
                    (
                        "TW_SEMI".to_string(),
                        vec![
                            IndicatorObservation {
                                date: date(2099, 1, 2),
                                value: Decimal::from_str_exact("67.89").expect("valid decimal"),
                            },
                            IndicatorObservation {
                                date: date(2099, 1, 5),
                                value: Decimal::from_str_exact("67.89").expect("valid decimal"),
                            },
                        ],
                    ),
                ]),
                errors: Vec::new(),
            },
        );
    }
}
