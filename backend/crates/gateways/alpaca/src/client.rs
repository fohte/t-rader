use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use core_application::bars::{
    SkippedUsStockInstrument, UsStockBarPage, UsStockBarQuery, UsStockBarSource,
    UsStockBarSourceError, UsStockSplitBatch, UsStockSplitQuery,
};
use core_domain::bar::Timeframe;
use core_domain::stock_id::ForeignStockId;
use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Url, header::HeaderValue};
use serde::de::DeserializeOwned;

mod response;

use response::{BarsResponse, CorporateActionsResponse};

const API_URL: &str = "https://data.alpaca.markets/v2/stocks/bars";
const CORPORATE_ACTIONS_RESPONSE_LIMIT: u32 = 1_000;
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
    #[error("US stock bar run deadline exceeded")]
    RunDeadlineExceeded,
}

enum AlpacaRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    TestNoop,
    #[cfg(test)]
    TestWaitExceeded,
}

impl AlpacaRateLimiter {
    async fn acquire(&self, timeout: Duration) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => {
                rate_limiter
                    .acquire(
                        &[Quota::new(
                            RATE_LIMIT_GLOBAL_KEY,
                            RATE_LIMIT_PER_MINUTE,
                            RATE_LIMIT_PERIOD,
                        )],
                        timeout,
                    )
                    .await
            }
            #[cfg(test)]
            Self::TestNoop => Ok(()),
            #[cfg(test)]
            Self::TestWaitExceeded => Err(RateLimitError::MaxWaitExceeded {
                max_wait: timeout + Duration::from_millis(1),
            }),
        }
    }

    async fn penalize(&self, duration: Duration) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => {
                rate_limiter.penalize(RATE_LIMIT_GLOBAL_KEY, duration).await
            }
            #[cfg(test)]
            Self::TestNoop => Ok(()),
            #[cfg(test)]
            Self::TestWaitExceeded => Ok(()),
        }
    }
}

pub struct AlpacaClient {
    client: reqwest::Client,
    rate_limiter: AlpacaRateLimiter,
    url: Url,
    corporate_actions_url: Url,
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
        let mut corporate_actions_url = url.clone();
        corporate_actions_url.set_path("/v1/corporate-actions");

        Ok(Self {
            client,
            rate_limiter,
            url,
            corporate_actions_url,
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
        url: &Url,
        params: &[(&'static str, String)],
        deadline: Instant,
    ) -> Result<Option<reqwest::Response>, AlpacaError> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(None);
        }
        match self.rate_limiter.acquire(remaining).await {
            Ok(()) => {}
            Err(RateLimitError::MaxWaitExceeded { .. }) => return Ok(None),
            Err(_) if Instant::now() >= deadline => return Ok(None),
            Err(error) => return Err(AlpacaError::RateLimit(error.to_string())),
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        let response = self
            .client
            .get(url.clone())
            .query(params)
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

        Ok(Some(response))
    }

    async fn fetch_json<T: DeserializeOwned>(
        &self,
        url: &Url,
        params: &[(&'static str, String)],
        deadline: Instant,
    ) -> Result<Option<T>, AlpacaError> {
        let Some(response) = self.fetch_response(url, params, deadline).await? else {
            return Ok(None);
        };
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

        response
            .json::<T>()
            .await
            .map(Some)
            .map_err(|error| AlpacaError::Parse(error.to_string()))
    }
}

#[async_trait]
impl UsStockBarSource for AlpacaClient {
    async fn fetch_page(
        &self,
        query: &UsStockBarQuery,
        deadline: Instant,
    ) -> Result<UsStockBarPage, UsStockBarSourceError> {
        self.fetch_page_inner(query, deadline)
            .await
            .map_err(source_error)
    }

    async fn fetch_splits(
        &self,
        query: &UsStockSplitQuery,
        max_requests: usize,
        deadline: Instant,
    ) -> Result<UsStockSplitBatch, UsStockBarSourceError> {
        self.fetch_splits_inner(query, max_requests, deadline)
            .await
            .map_err(source_error)
    }
}

impl AlpacaClient {
    async fn fetch_page_inner(
        &self,
        query: &UsStockBarQuery,
        deadline: Instant,
    ) -> Result<UsStockBarPage, AlpacaError> {
        if query.instrument_ids.is_empty() {
            return Ok(UsStockBarPage::default());
        }

        let (instrument_ids, symbols, skipped_instruments) = alpaca_symbols(&query.instrument_ids);
        if instrument_ids.is_empty() {
            return Ok(UsStockBarPage {
                skipped_instruments: if query.page_token.is_none() {
                    skipped_instruments
                } else {
                    Vec::new()
                },
                ..UsStockBarPage::default()
            });
        }
        let mut request_query = query.clone();
        request_query.instrument_ids = instrument_ids;

        let Some(response) = self
            .fetch_json::<BarsResponse>(&self.url, &request_params(&request_query)?, deadline)
            .await?
        else {
            return Err(AlpacaError::RunDeadlineExceeded);
        };

        let mut bars = Vec::new();
        for (symbol, records) in response.bars {
            let Some(instrument_id) = symbols.get(&symbol) else {
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
            skipped_instruments: if query.page_token.is_none() {
                skipped_instruments
            } else {
                Vec::new()
            },
        })
    }

    async fn fetch_splits_inner(
        &self,
        query: &UsStockSplitQuery,
        max_requests: usize,
        deadline: Instant,
    ) -> Result<UsStockSplitBatch, AlpacaError> {
        if query.instrument_ids.is_empty() {
            return Ok(UsStockSplitBatch::default());
        }
        if max_requests == 0 {
            return Ok(UsStockSplitBatch {
                has_more: true,
                ..UsStockSplitBatch::default()
            });
        }

        let (instrument_ids, symbols, skipped_instruments) = alpaca_symbols(&query.instrument_ids);
        if instrument_ids.is_empty() {
            return Ok(UsStockSplitBatch {
                skipped_instruments,
                ..UsStockSplitBatch::default()
            });
        }
        let mut request_query = query.clone();
        request_query.instrument_ids = instrument_ids;
        let params = split_request_params(&request_query)?;
        let mut page_token: Option<String> = None;
        let mut splits = Vec::new();
        let mut requests_attempted = 0;

        loop {
            let mut page_params = params.clone();
            if let Some(page_token) = &page_token {
                page_params.push(("page_token", page_token.clone()));
            }
            let Some(response) = self
                .fetch_json::<CorporateActionsResponse>(
                    &self.corporate_actions_url,
                    &page_params,
                    deadline,
                )
                .await?
            else {
                return Ok(UsStockSplitBatch {
                    splits,
                    requests_attempted,
                    has_more: true,
                    deadline_exceeded: true,
                    skipped_instruments,
                });
            };
            requests_attempted += 1;
            for split in response
                .forward_splits
                .into_iter()
                .chain(response.reverse_splits)
            {
                let Some(instrument_id) = symbols.get(&split.symbol) else {
                    return Err(AlpacaError::Parse(format!(
                        "response included an unrequested split symbol: {}",
                        split.symbol
                    )));
                };
                splits.push(split.into_split(instrument_id.clone())?);
            }

            let Some(next_page_token) = response.next_page_token else {
                return Ok(UsStockSplitBatch {
                    splits,
                    requests_attempted,
                    has_more: false,
                    deadline_exceeded: false,
                    skipped_instruments,
                });
            };
            if page_token.as_ref() == Some(&next_page_token) {
                return Err(AlpacaError::Parse(
                    "corporate actions pagination token did not advance".to_owned(),
                ));
            }
            if requests_attempted >= max_requests {
                return Ok(UsStockSplitBatch {
                    splits,
                    requests_attempted,
                    has_more: true,
                    deadline_exceeded: false,
                    skipped_instruments,
                });
            }
            page_token = Some(next_page_token);
        }
    }
}

fn source_error(error: AlpacaError) -> UsStockBarSourceError {
    match error {
        AlpacaError::RunDeadlineExceeded => UsStockBarSourceError::RunDeadlineExceeded,
        AlpacaError::Api { status, message } if is_symbol_rejection(status, &message) => {
            UsStockBarSourceError::RejectedSymbols(message)
        }
        error => UsStockBarSourceError::Failed(error.to_string()),
    }
}

fn is_symbol_rejection(status: u16, message: &str) -> bool {
    if !(400..500).contains(&status) || matches!(status, 401 | 403 | 429) {
        return false;
    }

    let message = message.to_ascii_lowercase();
    let identifies_instrument = ["symbol", "ticker", "asset", "instrument"]
        .iter()
        .any(|term| message.contains(term));
    let indicates_rejection = [
        "invalid",
        "unknown",
        "unsupported",
        "not supported",
        "not found",
        "cannot be found",
        "could not be found",
        "does not exist",
        "doesn't exist",
        "not recognized",
        "unrecognized",
        "not active",
        "inactive",
        "not tradable",
        "not tradeable",
        "no such",
        "rejected",
    ]
    .iter()
    .any(|term| message.contains(term));

    identifies_instrument && indicates_rejection
}

fn alpaca_symbols(
    instrument_ids: &[String],
) -> (
    Vec<String>,
    HashMap<String, String>,
    Vec<SkippedUsStockInstrument>,
) {
    let mut valid_instrument_ids = Vec::new();
    let mut symbols = HashMap::new();
    let mut skipped_instruments = Vec::new();
    for instrument_id in instrument_ids {
        match alpaca_symbol(instrument_id) {
            Ok(symbol) => {
                valid_instrument_ids.push(instrument_id.clone());
                symbols.insert(symbol, instrument_id.clone());
            }
            Err(error) => skipped_instruments.push(SkippedUsStockInstrument {
                instrument_id: instrument_id.clone(),
                reason: error.to_string(),
            }),
        }
    }
    (valid_instrument_ids, symbols, skipped_instruments)
}

fn split_request_params(
    query: &UsStockSplitQuery,
) -> Result<Vec<(&'static str, String)>, AlpacaError> {
    let symbols = query
        .instrument_ids
        .iter()
        .map(|instrument_id| alpaca_symbol(instrument_id))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(vec![
        ("symbols", symbols),
        ("types", "forward_split,reverse_split".to_owned()),
        ("start", query.from.to_string()),
        ("end", query.to.to_string()),
        ("limit", CORPORATE_ACTIONS_RESPONSE_LIMIT.to_string()),
        ("sort", "asc".to_owned()),
    ])
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
        Timeframe::Hourly => "1Hour",
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
        ("feed", "sip".to_owned()),
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use chrono::{DateTime, NaiveDate, Utc};
    use core_application::bars::{
        SkippedUsStockInstrument, UsStockBarPage, UsStockBarQuery, UsStockBarSource,
        UsStockBarSourceError, UsStockSplit, UsStockSplitBatch, UsStockSplitQuery,
    };
    use core_domain::bar::{Bar, Timeframe};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{AlpacaClient, AlpacaRateLimiter};

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .map(|timestamp| timestamp.to_utc())
            .unwrap_or_default()
    }

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(30)
    }

    #[rstest]
    #[tokio::test]
    async fn returns_deadline_exceeded_when_rate_limit_wait_exceeds_remaining_time() {
        let server = MockServer::start().await;
        let client = AlpacaClient::build(
            &format!("{}/v2/stocks/bars", server.uri()),
            "synthetic-key-id",
            "synthetic-secret",
            AlpacaRateLimiter::TestWaitExceeded,
        )
        .expect("test client is configured");
        let actual = (
            client
                .fetch_page(
                    &UsStockBarQuery {
                        instrument_ids: vec!["US:QZ7".to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: date("2040-01-03T00:00:00Z"),
                        page_token: None,
                    },
                    deadline(),
                )
                .await,
            server
                .received_requests()
                .await
                .map(|requests| requests.len()),
        );

        assert_eq!(
            actual,
            (Err(UsStockBarSourceError::RunDeadlineExceeded), Some(0)),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn does_not_send_bar_request_after_the_deadline() {
        let server = MockServer::start().await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");
        let actual = (
            client
                .fetch_page(
                    &UsStockBarQuery {
                        instrument_ids: vec!["US:QZ7".to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: date("2040-01-03T00:00:00Z"),
                        page_token: None,
                    },
                    Instant::now() - Duration::from_secs(1),
                )
                .await,
            server
                .received_requests()
                .await
                .map(|requests| requests.len()),
        );

        assert_eq!(
            actual,
            (Err(UsStockBarSourceError::RunDeadlineExceeded), Some(0)),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_split_adjusted_sip_bars_and_maps_class_share_symbols() {
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
            .and(query_param("feed", "sip"))
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
            .fetch_page(
                &UsStockBarQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned(), "KR:QZ7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: date("2040-01-03T00:00:00Z"),
                    page_token: None,
                },
                deadline(),
            )
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
                    adjustment_factor: Decimal::ONE,
                }],
                next_page_token: Some("synthetic-page".to_owned()),
                skipped_instruments: vec![SkippedUsStockInstrument {
                    instrument_id: "KR:QZ7".to_owned(),
                    reason:
                        "invalid Alpaca API configuration: stock is not in the US market: KR:QZ7"
                            .to_owned(),
                }],
            },
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_hourly_bars_using_the_hourly_timeframe() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/stocks/bars"))
            .and(query_param("symbols", "QZ7"))
            .and(query_param("timeframe", "1Hour"))
            .and(query_param("start", "2016-01-01T00:00:00+00:00"))
            .and(query_param("end", "2039-01-02T12:00:00+00:00"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "bars": {
                    "QZ7": [{
                        "t": "2039-01-02T12:00:00Z",
                        "o": 12.34,
                        "h": 14.56,
                        "l": 11.12,
                        "c": 13.45,
                        "v": 25
                    }]
                },
                "next_page_token": null
            })))
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let page = client
            .fetch_page(
                &UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Hourly,
                    from: date("2016-01-01T00:00:00Z"),
                    to: date("2039-01-02T12:00:00Z"),
                    page_token: None,
                },
                deadline(),
            )
            .await
            .expect("hourly bars response is valid");

        assert_eq!(
            page,
            core_application::bars::UsStockBarPage {
                bars: vec![Bar {
                    instrument_id: "US:QZ7".to_owned(),
                    timeframe: Timeframe::Hourly,
                    timestamp: date("2039-01-02T12:00:00Z"),
                    open: Decimal::new(1234, 2),
                    high: Decimal::new(1456, 2),
                    low: Decimal::new(1112, 2),
                    close: Decimal::new(1345, 2),
                    volume: 25,
                    adjustment_factor: Decimal::ONE,
                }],
                next_page_token: None,
                skipped_instruments: Vec::new(),
            },
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_forward_and_reverse_splits_for_requested_symbols() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/corporate-actions"))
            .and(header("APCA-API-KEY-ID", "synthetic-key-id"))
            .and(header("APCA-API-SECRET-KEY", "synthetic-secret"))
            .and(query_param("symbols", "QZ.7"))
            .and(query_param("types", "forward_split,reverse_split"))
            .and(query_param("start", "1970-01-01"))
            .and(query_param("end", "2040-01-03"))
            .and(query_param("limit", "1000"))
            .and(query_param("sort", "asc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "forward_splits": [{
                    "symbol": "QZ.7",
                    "ex_date": "2040-01-02",
                    "old_rate": "1",
                    "new_rate": 2
                }],
                "reverse_splits": [{
                    "symbol": "QZ.7",
                    "ex_date": "2039-01-02",
                    "old_rate": 10,
                    "new_rate": "1"
                }],
                "next_page_token": null
            })))
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let actual = client
            .fetch_splits(
                &UsStockSplitQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: NaiveDate::from_ymd_opt(2040, 1, 3).expect("end date is valid"),
                },
                1,
                deadline(),
            )
            .await
            .expect("corporate actions response is valid");

        assert_eq!(
            actual,
            UsStockSplitBatch {
                splits: vec![
                    UsStockSplit {
                        instrument_id: "US:QZ-7".to_owned(),
                        ex_date: NaiveDate::from_ymd_opt(2040, 1, 2).expect("split date is valid"),
                        old_rate: Decimal::ONE,
                        new_rate: Decimal::new(2, 0),
                    },
                    UsStockSplit {
                        instrument_id: "US:QZ-7".to_owned(),
                        ex_date: NaiveDate::from_ymd_opt(2039, 1, 2).expect("split date is valid"),
                        old_rate: Decimal::new(10, 0),
                        new_rate: Decimal::ONE,
                    },
                ],
                requests_attempted: 1,
                has_more: false,
                deadline_exceeded: false,
                skipped_instruments: Vec::new(),
            },
        );
    }

    #[rstest]
    #[case::stops_at_the_request_limit(1)]
    #[case::fetches_all_pages(2)]
    #[tokio::test]
    async fn respects_the_corporate_action_request_limit(#[case] max_requests: usize) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/corporate-actions"))
            .and(query_param_is_missing("page_token"))
            .and(query_param("symbols", "QZ.7"))
            .and(query_param("types", "forward_split,reverse_split"))
            .and(query_param("start", "1970-01-01"))
            .and(query_param("end", "2040-01-03"))
            .and(query_param("limit", "1000"))
            .and(query_param("sort", "asc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "forward_splits": [{
                    "symbol": "QZ.7",
                    "ex_date": "2040-01-02",
                    "old_rate": "1",
                    "new_rate": "2"
                }],
                "reverse_splits": [],
                "next_page_token": "synthetic-split-page"
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/corporate-actions"))
            .and(query_param("page_token", "synthetic-split-page"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "forward_splits": [],
                "reverse_splits": [{
                    "symbol": "QZ.7",
                    "ex_date": "2039-01-02",
                    "old_rate": 10,
                    "new_rate": 1
                }],
                "next_page_token": null
            })))
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let actual = client
            .fetch_splits(
                &UsStockSplitQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: NaiveDate::from_ymd_opt(2040, 1, 3).expect("end date is valid"),
                },
                max_requests,
                deadline(),
            )
            .await
            .expect("corporate actions pages are valid");

        let first_split = UsStockSplit {
            instrument_id: "US:QZ-7".to_owned(),
            ex_date: NaiveDate::from_ymd_opt(2040, 1, 2).expect("split date is valid"),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        };
        let second_split = UsStockSplit {
            instrument_id: "US:QZ-7".to_owned(),
            ex_date: NaiveDate::from_ymd_opt(2039, 1, 2).expect("split date is valid"),
            old_rate: Decimal::new(10, 0),
            new_rate: Decimal::ONE,
        };
        let expected = if max_requests == 1 {
            UsStockSplitBatch {
                splits: vec![first_split],
                requests_attempted: 1,
                has_more: true,
                deadline_exceeded: false,
                skipped_instruments: Vec::new(),
            }
        } else {
            UsStockSplitBatch {
                splits: vec![first_split, second_split],
                requests_attempted: 2,
                has_more: false,
                deadline_exceeded: false,
                skipped_instruments: Vec::new(),
            }
        };

        assert_eq!(actual, expected,);
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
            .fetch_page(
                &UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Minute,
                    from: date("2038-01-02T00:00:00Z"),
                    to: date("2040-01-03T00:00:00Z"),
                    page_token: Some("synthetic-page".to_owned()),
                },
                deadline(),
            )
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
    async fn skips_stock_ids_that_cannot_be_converted_to_alpaca_symbols(
        #[case] instrument_id: &str,
        #[case] expected: &str,
    ) {
        let server = MockServer::start().await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");
        let result = client
            .fetch_page(
                &UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: date("2040-01-03T00:00:00Z"),
                    page_token: None,
                },
                deadline(),
            )
            .await;

        assert_eq!(
            result,
            Ok(UsStockBarPage {
                bars: Vec::new(),
                next_page_token: None,
                skipped_instruments: vec![SkippedUsStockInstrument {
                    instrument_id: instrument_id.to_owned(),
                    reason: expected.to_owned(),
                }],
            }),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn stops_corporate_action_pagination_before_the_next_request_after_deadline() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/corporate-actions"))
            .and(query_param_is_missing("page_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(1_100))
                    .set_body_json(json!({
                        "forward_splits": [],
                        "reverse_splits": [],
                        "next_page_token": "synthetic-split-page"
                    })),
            )
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let actual = (
            client
                .fetch_splits(
                    &UsStockSplitQuery {
                        instrument_ids: vec!["US:QZ7".to_owned()],
                        from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                        to: NaiveDate::from_ymd_opt(2040, 1, 3).expect("end date is valid"),
                    },
                    2,
                    Instant::now() + Duration::from_secs(1),
                )
                .await,
            server
                .received_requests()
                .await
                .map(|requests| requests.len()),
        );

        assert_eq!(
            actual,
            (
                Ok(UsStockSplitBatch {
                    splits: Vec::new(),
                    requests_attempted: 1,
                    has_more: true,
                    deadline_exceeded: true,
                    skipped_instruments: Vec::new(),
                }),
                Some(1),
            ),
        );
    }

    #[rstest]
    #[case::invalid_symbol_400(400, "invalid symbol: QZ7", true)]
    #[case::unknown_asset_422(422, r#"{"code":42210000,"message":"asset QZ7 not found"}"#, true)]
    #[case::subscription_error_403(
        403,
        r#"{"message":"subscription does not permit querying recent SIP data"}"#,
        false
    )]
    #[tokio::test]
    async fn classifies_symbol_related_client_errors_for_batch_isolation(
        #[case] status: u16,
        #[case] response_body: &str,
        #[case] rejected_symbols: bool,
    ) {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/stocks/bars"))
            .and(query_param("symbols", "QZ7"))
            .respond_with(ResponseTemplate::new(status).set_body_string(response_body))
            .mount(&server)
            .await;
        let client = AlpacaClient::with_base_url(&format!("{}/v2/stocks/bars", server.uri()))
            .expect("test client is configured");

        let actual = client
            .fetch_page(
                &UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: date("2040-01-03T00:00:00Z"),
                    page_token: None,
                },
                deadline(),
            )
            .await;

        let expected = if rejected_symbols {
            Err(UsStockBarSourceError::RejectedSymbols(
                response_body.to_owned(),
            ))
        } else {
            Err(UsStockBarSourceError::Failed(format!(
                "Alpaca API returned HTTP {status}: {response_body}"
            )))
        };
        assert_eq!(actual, expected);
    }
}
