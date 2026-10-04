use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use core_application::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};
use core_domain::{stock_id::ForeignStockId, us_stock_master::UsStockMasterEntry};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use serde_json::Value;

const DEFAULT_ENDPOINT: &str = "https://www.sec.gov/files/company_tickers_exchange.json";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct SecClient {
    http: reqwest::Client,
    endpoint: String,
}

impl SecClient {
    pub fn new(user_agent: &str) -> Result<Self, UsStockMasterSourceError> {
        Self::with_endpoint(user_agent, DEFAULT_ENDPOINT)
    }

    pub fn with_endpoint(
        user_agent: &str,
        endpoint: &str,
    ) -> Result<Self, UsStockMasterSourceError> {
        let user_agent = HeaderValue::from_str(user_agent.trim()).map_err(|error| {
            UsStockMasterSourceError::Failed(format!("invalid SEC_USER_AGENT: {error}"))
        })?;
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, user_agent);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|error| {
                UsStockMasterSourceError::Failed(format!(
                    "failed to initialize SEC client: {error}"
                ))
            })?;

        Ok(Self {
            http,
            endpoint: endpoint.to_owned(),
        })
    }
}

#[async_trait]
impl UsStockMasterSource for SecClient {
    async fn fetch_all_us_stocks(
        &self,
    ) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError> {
        let response = self
            .http
            .get(&self.endpoint)
            .send()
            .await
            .map_err(|error| {
                UsStockMasterSourceError::Failed(format!(
                    "SEC request failed: {}",
                    error.without_url()
                ))
            })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(UsStockMasterSourceError::Failed(format!(
                "SEC returned HTTP status {status}"
            )));
        }

        let body = response.text().await.map_err(|error| {
            UsStockMasterSourceError::Failed(format!(
                "failed to read SEC response: {}",
                error.without_url()
            ))
        })?;
        parse_stock_master(&body)
    }
}

#[derive(Deserialize)]
struct RawStockMaster {
    fields: Vec<String>,
    data: Vec<Vec<Value>>,
}

fn parse_stock_master(body: &str) -> Result<Vec<UsStockMasterEntry>, UsStockMasterSourceError> {
    let response: RawStockMaster = serde_json::from_str(body).map_err(|error| {
        UsStockMasterSourceError::Failed(format!("invalid SEC response: {error}"))
    })?;
    let column_index = |name: &str| {
        response
            .fields
            .iter()
            .position(|field| field == name)
            .ok_or_else(|| {
                UsStockMasterSourceError::Failed(format!(
                    "SEC response is missing the {name} field"
                ))
            })
    };
    let ticker_index = column_index("ticker")?;
    let name_index = column_index("name")?;
    let exchange_index = column_index("exchange")?;
    let mut entries = Vec::with_capacity(response.data.len());
    let mut seen = HashSet::with_capacity(response.data.len());

    for (row_number, row) in response.data.into_iter().enumerate() {
        if row.len() != response.fields.len() {
            return Err(UsStockMasterSourceError::Failed(format!(
                "SEC response row {} has {} values, expected {}",
                row_number + 1,
                row.len(),
                response.fields.len()
            )));
        }
        let ticker = row[ticker_index].as_str().ok_or_else(|| {
            UsStockMasterSourceError::Failed(format!(
                "SEC response row {} has a non-string ticker",
                row_number + 1
            ))
        })?;
        let code = ticker.trim().replace('.', "-");
        let Ok(id) = ForeignStockId::new("US", &code) else {
            continue;
        };
        let name = row[name_index].as_str().ok_or_else(|| {
            UsStockMasterSourceError::Failed(format!(
                "SEC response row {} has a non-string name",
                row_number + 1
            ))
        })?;
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if !seen.insert(id.as_str().to_owned()) {
            continue;
        }
        let exchange = match &row[exchange_index] {
            Value::String(exchange) => {
                let exchange = exchange.trim();
                (!exchange.is_empty()).then(|| exchange.to_owned())
            }
            Value::Null => None,
            _ => {
                return Err(UsStockMasterSourceError::Failed(format!(
                    "SEC response row {} has an invalid exchange",
                    row_number + 1
                )));
            }
        };

        entries.push(UsStockMasterEntry {
            id,
            name: name.to_owned(),
            exchange,
        });
    }

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use core_application::us_stock_master_source::{UsStockMasterSource, UsStockMasterSourceError};
    use indoc::indoc;
    use rstest::rstest;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::SecClient;

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
                        ["重複した表記", "QZ-A", "Example Exchange", 900000004]
                    ]
                }
            "#}))
            .mount(&server)
            .await;
        let client = SecClient::with_endpoint(
            "Synthetic Client contact@fictional.invalid",
            &format!("{}/company_tickers_exchange.json", server.uri()),
        )
        .expect("client initializes");

        let stocks = client.fetch_all_us_stocks().await.expect("fetch succeeds");
        let actual = stocks
            .into_iter()
            .map(|stock| (stock.id.as_str().to_owned(), stock.name, stock.exchange))
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                (
                    "US:QZ-A".to_string(),
                    "架空クラス A".to_string(),
                    Some("Example Exchange".to_string()),
                ),
                (
                    "US:QZ-B".to_string(),
                    "架空銘柄 B".to_string(),
                    Some("Example Exchange".to_string()),
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
        let client = SecClient::with_endpoint(
            "Synthetic Client contact@fictional.invalid",
            &format!("{}/company_tickers_exchange.json", server.uri()),
        )
        .expect("client initializes");

        assert_eq!(
            client.fetch_all_us_stocks().await,
            Err(UsStockMasterSourceError::Failed(
                "SEC returned HTTP status 503".into(),
            )),
        );
    }
}
