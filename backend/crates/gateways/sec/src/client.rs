use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use core_application::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};
use core_domain::{stock_id::ForeignStockId, us_stock_master::UsStockMasterEntry};
use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Url, header::HeaderValue};
use serde::Deserialize;
use serde_json::Value;

use crate::error::SecClientError;

const DEFAULT_ENDPOINT: &str = "https://www.sec.gov/files/company_tickers_exchange.json";
const DEFAULT_DATA_ENDPOINT: &str = "https://data.sec.gov/";
const DEFAULT_ARCHIVE_ENDPOINT: &str = "https://www.sec.gov/Archives/edgar/data/";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_GLOBAL_KEY: &str = "sec:edgar";
const RATE_LIMIT_PER_SECOND: u32 = 5;
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(60);
const RATE_LIMIT_RETRY_DELAY: Duration = Duration::from_secs(60);

pub(super) enum SecRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    TestNoop,
}

impl SecRateLimiter {
    async fn acquire(&self) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => {
                rate_limiter
                    .acquire(
                        &[Quota::new(
                            RATE_LIMIT_GLOBAL_KEY,
                            RATE_LIMIT_PER_SECOND,
                            Duration::from_secs(1),
                        )],
                        RATE_LIMIT_MAX_WAIT,
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

pub struct SecClient {
    http: reqwest::Client,
    rate_limiter: SecRateLimiter,
    master_url: Url,
    data_url: Url,
    archive_url: Url,
}

impl SecClient {
    pub fn new(redis_url: &str, user_agent: &str) -> Result<Self, SecClientError> {
        let rate_limiter = RateLimiter::new(redis_url, RATE_LIMIT_KEY_PREFIX)
            .map_err(|error| SecClientError::RateLimit(error.to_string()))?;
        Self::build(
            user_agent,
            DEFAULT_ENDPOINT,
            DEFAULT_DATA_ENDPOINT,
            DEFAULT_ARCHIVE_ENDPOINT,
            SecRateLimiter::Shared(rate_limiter),
        )
    }

    pub(super) fn build(
        user_agent: &str,
        master_endpoint: &str,
        data_endpoint: &str,
        archive_endpoint: &str,
        rate_limiter: SecRateLimiter,
    ) -> Result<Self, SecClientError> {
        let user_agent = HeaderValue::from_str(user_agent.trim())
            .map_err(|error| SecClientError::Config(format!("invalid SEC_USER_AGENT: {error}")))?;
        let http = reqwest::Client::builder()
            .user_agent(user_agent.to_str().map_err(|error| {
                SecClientError::Config(format!("invalid SEC_USER_AGENT: {error}"))
            })?)
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| SecClientError::Config(error.to_string()))?;
        let master_url = Url::parse(master_endpoint)
            .map_err(|error| SecClientError::Config(error.to_string()))?;
        let data_url =
            Url::parse(data_endpoint).map_err(|error| SecClientError::Config(error.to_string()))?;
        let archive_url = Url::parse(archive_endpoint)
            .map_err(|error| SecClientError::Config(error.to_string()))?;

        Ok(Self {
            http,
            rate_limiter,
            master_url,
            data_url,
            archive_url,
        })
    }

    async fn get(&self, url: Url) -> Result<reqwest::Response, SecClientError> {
        self.rate_limiter
            .acquire()
            .await
            .map_err(|error| SecClientError::RateLimit(error.to_string()))?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|error| SecClientError::Request(error.without_url().to_string()))?;
        if response.status().as_u16() == 429 {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs)
                .filter(|duration| !duration.is_zero())
                .unwrap_or(RATE_LIMIT_RETRY_DELAY);
            self.rate_limiter
                .penalize(retry_after)
                .await
                .map_err(|error| SecClientError::RateLimit(error.to_string()))?;
        }
        Ok(response)
    }

    pub(crate) async fn get_text(&self, url: Url) -> Result<String, SecClientError> {
        let response = self.get(url).await?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(SecClientError::Status(status));
        }
        response
            .text()
            .await
            .map_err(|error| SecClientError::Request(error.without_url().to_string()))
    }

    pub(crate) async fn get_response(&self, url: Url) -> Result<reqwest::Response, SecClientError> {
        self.get(url).await
    }

    pub(crate) fn data_url(&self) -> &Url {
        &self.data_url
    }

    pub(crate) fn archive_url(&self) -> &Url {
        &self.archive_url
    }
}

#[async_trait]
impl UsStockMasterSource for SecClient {
    async fn fetch_all_us_stocks(
        &self,
    ) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError> {
        let body = self
            .get_text(self.master_url.clone())
            .await
            .map_err(|error| UsStockMasterSourceError::Failed(error.to_string()))?;
        parse_stock_master(&body)
            .map_err(|error| UsStockMasterSourceError::Failed(error.to_string()))
    }
}

#[derive(Deserialize)]
struct RawStockMaster {
    fields: Vec<String>,
    data: Vec<Vec<Value>>,
}

fn parse_stock_master(body: &str) -> Result<Vec<UsStockMasterEntry>, SecClientError> {
    let response: RawStockMaster =
        serde_json::from_str(body).map_err(|error| SecClientError::Parse(error.to_string()))?;
    let column_index = |name: &str| {
        response
            .fields
            .iter()
            .position(|field| field == name)
            .ok_or_else(|| {
                SecClientError::Parse(format!("SEC response is missing the {name} field"))
            })
    };
    let ticker_index = column_index("ticker")?;
    let name_index = column_index("name")?;
    let exchange_index = column_index("exchange")?;
    let cik_index = column_index("cik")?;
    let mut entries = Vec::with_capacity(response.data.len());
    let mut entry_indexes = HashMap::with_capacity(response.data.len());
    let mut cik_candidates: HashMap<String, HashSet<Option<String>>> =
        HashMap::with_capacity(response.data.len());

    for (row_number, row) in response.data.into_iter().enumerate() {
        if row.len() != response.fields.len() {
            return Err(SecClientError::Parse(format!(
                "SEC response row {} has {} values, expected {}",
                row_number + 1,
                row.len(),
                response.fields.len()
            )));
        }
        let ticker = row[ticker_index].as_str().ok_or_else(|| {
            SecClientError::Parse(format!(
                "SEC response row {} has a non-string ticker",
                row_number + 1
            ))
        })?;
        let code = ticker.trim().replace('.', "-");
        let Ok(id) = ForeignStockId::new("US", &code) else {
            continue;
        };
        let name = row[name_index].as_str().ok_or_else(|| {
            SecClientError::Parse(format!(
                "SEC response row {} has a non-string name",
                row_number + 1
            ))
        })?;
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let exchange = match &row[exchange_index] {
            Value::String(exchange) => {
                let exchange = exchange.trim();
                (!exchange.is_empty()).then(|| exchange.to_owned())
            }
            Value::Null => None,
            _ => {
                return Err(SecClientError::Parse(format!(
                    "SEC response row {} has an invalid exchange",
                    row_number + 1
                )));
            }
        };
        let key = id.as_str().to_owned();
        let cik = parse_cik(&row[cik_index]);
        cik_candidates
            .entry(key.clone())
            .or_default()
            .insert(cik.clone());

        if entry_indexes.contains_key(&key) {
            continue;
        }
        entry_indexes.insert(key, entries.len());

        entries.push(UsStockMasterEntry {
            id,
            name: name.to_owned(),
            exchange,
            cik,
        });
    }

    for (id, entry_index) in entry_indexes {
        let candidates = cik_candidates.get(&id);
        entries[entry_index].cik = candidates
            .filter(|candidates| candidates.len() == 1)
            .and_then(|candidates| candidates.iter().next())
            .cloned()
            .flatten();
    }

    Ok(entries)
}

fn parse_cik(value: &Value) -> Option<String> {
    let cik = match value {
        Value::Number(value) => value.as_u64()?.to_string(),
        Value::String(value) => value.trim().to_owned(),
        _ => return None,
    };
    if cik.is_empty() || cik.len() > 10 || !cik.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let cik = cik.trim_start_matches('0');
    (!cik.is_empty()).then(|| cik.to_owned())
}

#[cfg(test)]
mod tests {
    use core_application::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};
    use indoc::indoc;
    use rstest::rstest;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{SecClient, SecRateLimiter};

    fn client(server: &MockServer) -> SecClient {
        let base = format!("{}/", server.uri());
        SecClient::build(
            "Synthetic Client contact@fictional.invalid",
            &format!("{base}company_tickers_exchange.json"),
            &base,
            &format!("{base}Archives/edgar/data/"),
            SecRateLimiter::TestNoop,
        )
        .expect("client initializes")
    }

    #[rstest]
    #[tokio::test]
    async fn fetches_and_normalizes_valid_stock_ids() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/company_tickers_exchange.json"))
            .and(header(
                "user-agent",
                "Synthetic Client contact@fictional.invalid",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(indoc! {r#"
                {
                    "fields": ["name", "ticker", "exchange", "cik"],
                    "data": [
                        ["架空クラス A", "QZ.A", "Example Exchange", 900000001],
                        ["架空銘柄 B", "QZ-B", "Example Exchange", 900000002],
                        ["無効な記号", "QZ?C", "Example Exchange", 900000003],
                        ["重複した表記", "QZ-A", "Example Exchange", 900000004],
                        ["架空銘柄 C", "QZ-C", "Example Exchange", null]
                    ]
                }
            "#}))
            .mount(&server)
            .await;
        let client = client(&server);

        let stocks = client.fetch_all_us_stocks().await.expect("fetch succeeds");
        let actual = stocks
            .into_iter()
            .map(|stock| {
                (
                    stock.id.as_str().to_owned(),
                    stock.name,
                    stock.exchange,
                    stock.cik,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                (
                    "US:QZ-A".to_string(),
                    "架空クラス A".to_string(),
                    Some("Example Exchange".to_string()),
                    None,
                ),
                (
                    "US:QZ-B".to_string(),
                    "架空銘柄 B".to_string(),
                    Some("Example Exchange".to_string()),
                    Some("900000002".to_string()),
                ),
                (
                    "US:QZ-C".to_string(),
                    "架空銘柄 C".to_string(),
                    Some("Example Exchange".to_string()),
                    None,
                ),
            ],
        );
    }

    #[rstest]
    #[tokio::test]
    async fn returns_an_error_when_sec_responds_with_a_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/company_tickers_exchange.json"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = client(&server);

        assert_eq!(
            client.fetch_all_us_stocks().await,
            Err(UsStockMasterSourceError::Failed(
                "SEC returned HTTP status 503".into(),
            )),
        );
    }
}
