use std::{collections::HashMap, str::FromStr, time::Duration};

use async_trait::async_trait;
use chrono::DateTime;
use core_application::bars::{
    UsStockBarPage, UsStockBarQuery, UsStockBarSource, UsStockBarSourceError,
};
use core_domain::bar::{Bar, Timeframe};
use core_domain::stock_id::ForeignStockId;
use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Url, header::HeaderValue};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::Deserialize;

const API_URL: &str = "https://data.alpaca.markets/v2/stocks/bars";
const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_GLOBAL_KEY: &str = "alpaca:market-data";
const RATE_LIMIT_PER_MINUTE: u32 = 150;
const RATE_LIMIT_PERIOD: Duration = Duration::from_secs(60);
const RATE_LIMIT_RETRY_AFTER: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_LIMIT: u32 = 10_000;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AlpacaError {
    #[error("invalid Alpaca API configuration: {0}")]
    Config(String),
    #[error("Alpaca request failed: {0}")]
    Request(String),
    #[error("Alpaca API returned HTTP {status}: {message}")]
    Api { status: u16, message: String },
    #[error("failed to parse Alpaca response: {0}")]
    Parse(String),
    #[error("Alpaca shared rate limit failed: {0}")]
    RateLimit(String),
}

enum AlpacaRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    TestNoop,
}

impl AlpacaRateLimiter {
    async fn acquire(&self) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => {
                rate_limiter
                    .acquire(
                        &[Quota::new(
                            RATE_LIMIT_GLOBAL_KEY,
                            RATE_LIMIT_PER_MINUTE,
                            RATE_LIMIT_PERIOD,
                        )],
                        Duration::MAX,
                    )
                    .await
            }
            #[cfg(test)]
            Self::TestNoop => Ok(()),
        }
    }

    async fn penalize(&self, duration: Duration) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => {
                rate_limiter.penalize(RATE_LIMIT_GLOBAL_KEY, duration).await
            }
            #[cfg(test)]
            Self::TestNoop => Ok(()),
        }
    }
}

pub struct AlpacaClient {
    client: reqwest::Client,
    rate_limiter: AlpacaRateLimiter,
    url: Url,
}

impl AlpacaClient {
    pub fn new(redis_url: &str, api_key_id: &str, secret_key: &str) -> Result<Self, AlpacaError> {
        let rate_limiter = RateLimiter::new(redis_url, RATE_LIMIT_KEY_PREFIX)
            .map_err(|error| AlpacaError::RateLimit(error.to_string()))?;
        Self::build(
            API_URL,
            api_key_id,
            secret_key,
            AlpacaRateLimiter::Shared(rate_limiter),
        )
    }

    fn build(
        url: &str,
        api_key_id: &str,
        secret_key: &str,
        rate_limiter: AlpacaRateLimiter,
    ) -> Result<Self, AlpacaError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "APCA-API-KEY-ID",
            HeaderValue::from_str(api_key_id)
                .map_err(|error| AlpacaError::Config(error.to_string()))?,
        );
        headers.insert(
            "APCA-API-SECRET-KEY",
            HeaderValue::from_str(secret_key)
                .map_err(|error| AlpacaError::Config(error.to_string()))?,
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| AlpacaError::Request(error.to_string()))?;
        let url = Url::parse(url).map_err(|error| AlpacaError::Config(error.to_string()))?;

        Ok(Self {
            client,
            rate_limiter,
            url,
        })
    }

    #[cfg(test)]
    fn with_base_url(url: &str) -> Result<Self, AlpacaError> {
        Self::build(
            url,
            "synthetic-key-id",
            "synthetic-secret",
            AlpacaRateLimiter::TestNoop,
        )
    }

    async fn fetch_response(
        &self,
        query: &UsStockBarQuery,
    ) -> Result<reqwest::Response, AlpacaError> {
        self.rate_limiter
            .acquire()
            .await
            .map_err(|error| AlpacaError::RateLimit(error.to_string()))?;
        let response = self
            .client
            .get(self.url.clone())
            .query(&request_params(query)?)
            .send()
            .await
            .map_err(|error| AlpacaError::Request(error.to_string()))?;

        if response.status().as_u16() == 429 {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or(RATE_LIMIT_RETRY_AFTER);
            self.rate_limiter
                .penalize(retry_after)
                .await
                .map_err(|error| AlpacaError::RateLimit(error.to_string()))?;
        }

        Ok(response)
    }
}

#[async_trait]
impl UsStockBarSource for AlpacaClient {
    async fn fetch_page(
        &self,
        query: &UsStockBarQuery,
    ) -> Result<UsStockBarPage, UsStockBarSourceError> {
        self.fetch_page_inner(query)
            .await
            .map_err(|error| UsStockBarSourceError::Failed(error.to_string()))
    }
}

impl AlpacaClient {
    async fn fetch_page_inner(
        &self,
        query: &UsStockBarQuery,
    ) -> Result<UsStockBarPage, AlpacaError> {
        if query.instrument_ids.is_empty() {
            return Ok(UsStockBarPage::default());
        }

        let response = self.fetch_response(query).await?;
        let status = response.status();
        if !status.is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|error| error.to_string());
            return Err(AlpacaError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let response = response
            .json::<BarsResponse>()
            .await
            .map_err(|error| AlpacaError::Parse(error.to_string()))?;
        let mut instrument_ids = HashMap::new();
        for instrument_id in &query.instrument_ids {
            instrument_ids.insert(alpaca_symbol(instrument_id)?, instrument_id.clone());
        }

        let mut bars = Vec::new();
        for (symbol, records) in response.bars {
            let Some(instrument_id) = instrument_ids.get(&symbol) else {
                return Err(AlpacaError::Parse(format!(
                    "response included an unrequested symbol: {symbol}"
                )));
            };
            for record in records {
                bars.push(record.into_bar(instrument_id, query.timeframe)?);
            }
        }

        Ok(UsStockBarPage {
            bars,
            next_page_token: response.next_page_token,
        })
    }
}

fn request_params(query: &UsStockBarQuery) -> Result<Vec<(&'static str, String)>, AlpacaError> {
    let symbols = query
        .instrument_ids
        .iter()
        .map(|instrument_id| alpaca_symbol(instrument_id))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let timeframe = match query.timeframe {
        Timeframe::Daily => "1Day",
        Timeframe::Minute => "1Min",
        timeframe => {
            return Err(AlpacaError::Config(format!(
                "unsupported bar timeframe: {timeframe}"
            )));
        }
    };
    let mut params = vec![
        ("symbols", symbols),
        ("timeframe", timeframe.to_owned()),
        ("start", query.from.to_rfc3339()),
        ("end", query.to.to_rfc3339()),
        ("adjustment", "split".to_owned()),
        ("feed", "iex".to_owned()),
        ("limit", RESPONSE_LIMIT.to_string()),
        ("sort", "asc".to_owned()),
    ];
    if let Some(page_token) = &query.page_token {
        params.push(("page_token", page_token.clone()));
    }
    Ok(params)
}

fn alpaca_symbol(instrument_id: &str) -> Result<String, AlpacaError> {
    let Some((country, ticker)) = instrument_id.split_once(':') else {
        return Err(AlpacaError::Config(format!(
            "US stock ID has no country prefix: {instrument_id}"
        )));
    };
    ForeignStockId::new(country, ticker).map_err(|error| AlpacaError::Config(error.to_string()))?;
    if country != "US" {
        return Err(AlpacaError::Config(format!(
            "stock is not in the US market: {instrument_id}"
        )));
    }
    Ok(ticker.replace('-', "."))
}

#[derive(Deserialize)]
struct BarsResponse {
    #[serde(default)]
    bars: HashMap<String, Vec<AlpacaBar>>,
    next_page_token: Option<String>,
}

#[derive(Deserialize)]
struct AlpacaBar {
    t: String,
    o: serde_json::Number,
    h: serde_json::Number,
    l: serde_json::Number,
    c: serde_json::Number,
    v: serde_json::Number,
}

impl AlpacaBar {
    fn into_bar(self, instrument_id: &str, timeframe: Timeframe) -> Result<Bar, AlpacaError> {
        let timestamp = DateTime::parse_from_rfc3339(&self.t)
            .map_err(|error| AlpacaError::Parse(error.to_string()))?
            .to_utc();
        let volume = decimal_from_number(self.v)?;
        if volume.fract() != Decimal::ZERO {
            return Err(AlpacaError::Parse(format!(
                "bar volume must be an integer: {volume}"
            )));
        }

        Ok(Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: decimal_from_number(self.o)?,
            high: decimal_from_number(self.h)?,
            low: decimal_from_number(self.l)?,
            close: decimal_from_number(self.c)?,
            volume: volume.to_i64().ok_or_else(|| {
                AlpacaError::Parse(format!("bar volume is out of range: {volume}"))
            })?,
        })
    }
}

fn decimal_from_number(value: serde_json::Number) -> Result<Decimal, AlpacaError> {
    Decimal::from_str(&value.to_string()).map_err(|error| AlpacaError::Parse(error.to_string()))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use core_application::bars::{UsStockBarQuery, UsStockBarSource, UsStockBarSourceError};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::AlpacaClient;

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .map(|timestamp| timestamp.to_utc())
            .unwrap_or_default()
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_split_adjusted_iex_bars_and_maps_class_share_symbols() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/stocks/bars"))
            .and(header("APCA-API-KEY-ID", "synthetic-key-id"))
            .and(header("APCA-API-SECRET-KEY", "synthetic-secret"))
            .and(query_param("symbols", "QZ.7"))
            .and(query_param("timeframe", "1Day"))
            .and(query_param("start", "1970-01-01T00:00:00+00:00"))
            .and(query_param("end", "2040-01-03T00:00:00+00:00"))
            .and(query_param("adjustment", "split"))
            .and(query_param("feed", "iex"))
            .and(query_param("limit", "10000"))
            .and(query_param("sort", "asc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "bars": {
                    "QZ.7": [{
                        "t": "2040-01-02T00:00:00Z",
                        "o": 12.34,
                        "h": 14.56,
                        "l": 11.12,
                        "c": 13.45,
                        "v": 25
                    }]
                },
                "next_page_token": "synthetic-page"
            })))
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let page = client
            .fetch_page(&UsStockBarQuery {
                instrument_ids: vec!["US:QZ-7".to_owned()],
                timeframe: Timeframe::Daily,
                from: date("1970-01-01T00:00:00Z"),
                to: date("2040-01-03T00:00:00Z"),
                page_token: None,
            })
            .await
            .expect("bars response is valid");

        assert_eq!(
            page,
            core_application::bars::UsStockBarPage {
                bars: vec![Bar {
                    instrument_id: "US:QZ-7".to_owned(),
                    timeframe: Timeframe::Daily,
                    timestamp: date("2040-01-02T00:00:00Z"),
                    open: Decimal::new(1234, 2),
                    high: Decimal::new(1456, 2),
                    low: Decimal::new(1112, 2),
                    close: Decimal::new(1345, 2),
                    volume: 25,
                }],
                next_page_token: Some("synthetic-page".to_owned()),
            },
        );
    }

    #[rstest]
    #[tokio::test]
    async fn sends_the_page_token_on_subsequent_requests() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/stocks/bars"))
            .and(query_param("page_token", "synthetic-page"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "bars": {},
                "next_page_token": null
            })))
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let page = client
            .fetch_page(&UsStockBarQuery {
                instrument_ids: vec!["US:QZ7".to_owned()],
                timeframe: Timeframe::Minute,
                from: date("2038-01-02T00:00:00Z"),
                to: date("2040-01-03T00:00:00Z"),
                page_token: Some("synthetic-page".to_owned()),
            })
            .await
            .expect("subsequent page response is valid");

        assert_eq!(page, core_application::bars::UsStockBarPage::default());
    }

    #[rstest]
    #[case::missing_prefix(
        "QZ7",
        "invalid Alpaca API configuration: US stock ID has no country prefix: QZ7"
    )]
    #[case::wrong_market(
        "KR:QZ7",
        "invalid Alpaca API configuration: stock is not in the US market: KR:QZ7"
    )]
    #[tokio::test]
    async fn rejects_stock_ids_that_are_not_us_prefixed(
        #[case] instrument_id: &str,
        #[case] expected: &str,
    ) {
        let server = MockServer::start().await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");
        let result = client
            .fetch_page(&UsStockBarQuery {
                instrument_ids: vec![instrument_id.to_owned()],
                timeframe: Timeframe::Daily,
                from: date("1970-01-01T00:00:00Z"),
                to: date("2040-01-03T00:00:00Z"),
                page_token: None,
            })
            .await;

        assert_eq!(
            result,
            Err(UsStockBarSourceError::Failed(expected.to_owned()))
        );
    }
}
