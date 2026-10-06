//! EODData API から株価指数の日足を取得するクライアント。

use async_trait::async_trait;
use chrono::{Duration, NaiveDate, Utc};
use chrono_tz::Asia::Seoul;
use core_application::indicator_observation_source::{
    IndicatorObservationSource, IndicatorObservationSourceError,
};
use core_domain::IndicatorObservation;
use reqwest::Url;
use rust_decimal::Decimal;
use serde::Deserialize;

const DEFAULT_BASE_URL: &str = "https://api.eoddata.com";
const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const HISTORY_DAYS: i64 = 30;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Quote {
    date_stamp: String,
    close: serde_json::Number,
}

pub struct EodDataClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl EodDataClient {
    pub fn new(api_key: String) -> Result<Self, IndicatorObservationSourceError> {
        Self::with_base_url(api_key, DEFAULT_BASE_URL)
    }

    pub fn with_base_url(
        api_key: String,
        base_url: &str,
    ) -> Result<Self, IndicatorObservationSourceError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| IndicatorObservationSourceError::Initialization(error.to_string()))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        })
    }

    fn build_url(
        &self,
        symbol_code: &str,
        observation_start: Option<NaiveDate>,
        today: NaiveDate,
    ) -> Result<Url, IndicatorObservationSourceError> {
        let mut url =
            Url::parse(&format!("{}/Quote/List/INDEX", self.base_url)).map_err(|error| {
                IndicatorObservationSourceError::Parse(format!("invalid base URL: {error}"))
            })?;
        url.path_segments_mut()
            .map_err(|()| {
                IndicatorObservationSourceError::Parse(
                    "base URL cannot accept path segments".to_string(),
                )
            })?
            .push(symbol_code);

        let earliest_available = today - Duration::days(HISTORY_DAYS - 1);
        let from_date = observation_start
            .map(|date| date.max(earliest_available))
            .unwrap_or(earliest_available);

        let mut query = url.query_pairs_mut();
        query
            .append_pair("ApiKey", &self.api_key)
            .append_pair("Interval", "d")
            .append_pair("FromDateStamp", &from_date.format("%Y-%m-%d").to_string())
            .append_pair("ToDateStamp", &today.format("%Y-%m-%d").to_string());
        drop(query);

        Ok(url)
    }
}

#[async_trait]
impl IndicatorObservationSource for EodDataClient {
    async fn fetch_observations(
        &self,
        series_id: &str,
        observation_start: Option<NaiveDate>,
    ) -> Result<Vec<IndicatorObservation>, IndicatorObservationSourceError> {
        let today = Utc::now().with_timezone(&Seoul).date_naive();
        let url = self.build_url(series_id, observation_start, today)?;
        let response = self.http.get(url).send().await.map_err(|error| {
            IndicatorObservationSourceError::Network(error.without_url().to_string())
        })?;

        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(IndicatorObservationSourceError::Api {
                status,
                message: format!("EODData returned status {status} for INDEX/{series_id}"),
            });
        }

        let body = response.text().await.map_err(|error| {
            IndicatorObservationSourceError::Network(error.without_url().to_string())
        })?;
        parse_quotes(&body)
    }
}

fn parse_quotes(body: &str) -> Result<Vec<IndicatorObservation>, IndicatorObservationSourceError> {
    let quotes: Vec<Quote> = serde_json::from_str(body).map_err(|error| {
        IndicatorObservationSourceError::Parse(format!("EODData response: {error}"))
    })?;

    quotes
        .into_iter()
        .map(|quote| {
            let date_text = quote
                .date_stamp
                .split_whitespace()
                .next()
                .unwrap_or_default();
            let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|error| {
                IndicatorObservationSourceError::Parse(format!(
                    "invalid EODData date '{}': {error}",
                    quote.date_stamp
                ))
            })?;
            let value = Decimal::from_str_exact(&quote.close.to_string()).map_err(|error| {
                IndicatorObservationSourceError::Parse(format!(
                    "invalid EODData close for {date}: {error}"
                ))
            })?;

            Ok(IndicatorObservation { date, value })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use core_application::indicator_observation_source::IndicatorObservationSource;
    use core_domain::IndicatorObservation;
    use indoc::indoc;
    use rstest::rstest;
    use rust_decimal::Decimal;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{EodDataClient, parse_quotes};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[rstest]
    #[case::initial_backfill(None, "2099-01-01")]
    #[case::within_history(Some(date(2099, 1, 15)), "2099-01-15")]
    #[case::outside_history(Some(date(2098, 12, 31)), "2099-01-01")]
    fn build_url_requests_only_available_daily_history(
        #[case] observation_start: Option<NaiveDate>,
        #[case] expected_from: &str,
    ) {
        let client = EodDataClient::with_base_url(
            "example-key".to_string(),
            "https://eoddata.example.test/",
        )
        .expect("client");
        let url = client
            .build_url("KSIC", observation_start, date(2099, 1, 30))
            .expect("URL");

        assert_eq!(
            url.as_str(),
            format!(
                "https://eoddata.example.test/Quote/List/INDEX/KSIC?ApiKey=example-key&Interval=d&FromDateStamp={expected_from}&ToDateStamp=2099-01-30"
            ),
        );
    }

    #[test]
    fn parse_quotes_converts_dates_and_numeric_closes_to_observations() {
        let body = indoc! {r#"
            [
              {"exchangeCode":"INDEX","symbolCode":"KSIC","interval":"d","dateStamp":"2099-01-29","open":1230.12,"high":1250.56,"low":1220.01,"close":1245.67,"volume":0},
              {"exchangeCode":"INDEX","symbolCode":"KSIC","interval":"d","dateStamp":"2099-01-30 16:00","open":1245.67,"high":1260.78,"low":1235.43,"close":1255.50,"volume":0}
            ]
        "#};

        assert_eq!(
            parse_quotes(body).expect("parse quotes"),
            vec![
                IndicatorObservation {
                    date: date(2099, 1, 29),
                    value: Decimal::from_str_exact("1245.67").expect("valid decimal"),
                },
                IndicatorObservation {
                    date: date(2099, 1, 30),
                    value: Decimal::from_str_exact("1255.50").expect("valid decimal"),
                },
            ],
        );
    }

    #[tokio::test]
    async fn fetch_observations_uses_daily_quote_endpoint_and_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Quote/List/INDEX/KSIC"))
            .and(query_param("ApiKey", "example-key"))
            .and(query_param("Interval", "d"))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                [{"dateStamp":"2099-01-30","close":1255.50}]
            "#}))
            .expect(1)
            .mount(&server)
            .await;
        let client =
            EodDataClient::with_base_url("example-key".to_string(), &server.uri()).expect("client");

        let observations = client
            .fetch_observations("KSIC", Some(date(2099, 1, 30)))
            .await
            .expect("fetch observations");

        assert_eq!(
            observations,
            vec![IndicatorObservation {
                date: date(2099, 1, 30),
                value: Decimal::from_str_exact("1255.50").expect("valid decimal"),
            }],
        );
    }

    #[tokio::test]
    async fn fetch_observations_does_not_include_the_api_key_in_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Quote/List/INDEX/KSIC"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        let client = EodDataClient::with_base_url("private-example-key".to_string(), &server.uri())
            .expect("client");
        let result = client
            .fetch_observations("KSIC", Some(date(2099, 1, 30)))
            .await
            .map(|observations| observations.len())
            .map_err(|error| error.to_string());

        assert_eq!(
            result,
            Err("api error (status 401): EODData returned status 401 for INDEX/KSIC".to_string()),
        );
    }
}
