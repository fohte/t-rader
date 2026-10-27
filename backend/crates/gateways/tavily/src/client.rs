use std::time::Duration;

use async_trait::async_trait;
use core_application::tavily_retrieval_cache::{
    EXTRACT_DEPTH, EXTRACT_FORMAT, EXTRACT_INCLUDE_USAGE, SEARCH_DEPTH,
    SEARCH_INCLUDE_PUBLISHED_DATE, SEARCH_INCLUDE_USAGE, SEARCH_MAX_RESULTS,
    TavilyCredentialGeneration,
};
use core_application::web_search::{
    WebPageExtractionFailure, WebPageExtractionResponse, WebSearchAccountUsage, WebSearchClient,
    WebSearchError, WebSearchResponse, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

const SEARCH_ENDPOINT: &str = "https://api.tavily.com/search";
const EXTRACT_ENDPOINT: &str = "https://api.tavily.com/extract";
const USAGE_ENDPOINT: &str = "https://api.tavily.com/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct TavilyClient {
    http: Client,
    search_endpoint: String,
    extract_endpoint: String,
    usage_endpoint: String,
    credential_generation: TavilyCredentialGeneration,
}

impl TavilyClient {
    pub fn from_env() -> Option<Self> {
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    fn from_env_with<F>(get: F) -> Option<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let Some(api_key) = get("TAVILY_API_KEY").filter(|value| !value.trim().is_empty()) else {
            tracing::warn!("TAVILY_API_KEY が未設定のため、Tavily client を無効化します");
            return None;
        };
        match Self::with_endpoints(
            SEARCH_ENDPOINT.to_owned(),
            EXTRACT_ENDPOINT.to_owned(),
            USAGE_ENDPOINT.to_owned(),
            &api_key,
        ) {
            Ok(client) => Some(client),
            Err(error) => {
                tracing::warn!(error = %error, "failed to initialize Tavily client");
                None
            }
        }
    }

    fn with_endpoints(
        search_endpoint: String,
        extract_endpoint: String,
        usage_endpoint: String,
        api_key: &str,
    ) -> Result<Self, WebSearchError> {
        let mut headers = HeaderMap::new();
        let authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|error| WebSearchError::Init(format!("invalid API key: {error}")))?;
        headers.insert(AUTHORIZATION, authorization);

        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .default_headers(headers)
            .build()
            .map_err(|error| {
                WebSearchError::Init(format!("failed to build HTTP client: {error}"))
            })?;

        Ok(Self {
            http,
            search_endpoint,
            extract_endpoint,
            usage_endpoint,
            credential_generation: TavilyCredentialGeneration::from_secret(api_key.as_bytes()),
        })
    }

    pub fn credential_generation(&self) -> TavilyCredentialGeneration {
        self.credential_generation
    }
}

fn network_error(error: reqwest::Error) -> WebSearchError {
    if error.is_timeout() {
        WebSearchError::NetworkTimeout
    } else {
        WebSearchError::Network(error.to_string())
    }
}

fn usage_error(error: reqwest::Error) -> WebSearchError {
    if error.is_timeout() {
        WebSearchError::UsageTimeout
    } else {
        WebSearchError::Usage(error.to_string())
    }
}

#[async_trait]
impl WebSearchClient for TavilyClient {
    async fn search(
        &self,
        query: &str,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<WebSearchResponse, WebSearchError> {
        let response = self
            .http
            .post(&self.search_endpoint)
            .json(&SearchRequest {
                query,
                search_depth: SEARCH_DEPTH,
                max_results: usize::from(SEARCH_MAX_RESULTS),
                topic: topic.map(WebSearchTopic::as_str),
                time_range: time_range.map(WebSearchTimeRange::as_str),
                include_published_date: SEARCH_INCLUDE_PUBLISHED_DATE,
                include_usage: SEARCH_INCLUDE_USAGE,
            })
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(WebSearchError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let body: SearchResponse = response
            .json()
            .await
            .map_err(|error| WebSearchError::Parse(error.to_string()))?;
        Ok(WebSearchResponse {
            results: body
                .results
                .into_iter()
                .map(SearchResult::into_domain)
                .collect(),
            credits_used: body.usage.map(|usage| usage.credits),
        })
    }

    async fn extract_page(&self, url: &str) -> Result<WebPageExtractionResponse, WebSearchError> {
        let response = self
            .http
            .post(&self.extract_endpoint)
            .json(&ExtractRequest {
                urls: [url],
                extract_depth: EXTRACT_DEPTH,
                format: EXTRACT_FORMAT,
                include_usage: EXTRACT_INCLUDE_USAGE,
            })
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(WebSearchError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let body: ExtractResponse = response
            .json()
            .await
            .map_err(|error| WebSearchError::Parse(error.to_string()))?;
        let extraction = match (
            body.results.into_iter().next(),
            body.failed_results.into_iter().next(),
        ) {
            (Some(result), None) => WebPageExtractionResponse {
                raw_content: Some(result.raw_content),
                failure: None,
                credits_used: body.usage.map(|usage| usage.credits),
            },
            (None, Some(failure)) => WebPageExtractionResponse {
                raw_content: None,
                failure: Some(WebPageExtractionFailure {
                    url: failure.url,
                    message: failure.error,
                }),
                credits_used: body.usage.map(|usage| usage.credits),
            },
            _ => {
                return Err(WebSearchError::Parse(
                    "Tavily extract response contains no result".to_string(),
                ));
            }
        };

        Ok(extraction)
    }

    async fn account_usage(&self) -> Result<WebSearchAccountUsage, WebSearchError> {
        let response = self
            .http
            .get(&self.usage_endpoint)
            .send()
            .await
            .map_err(usage_error)?;
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(WebSearchError::Usage(format!(
                "status {}: {message}",
                status.as_u16()
            )));
        }
        let body: AccountUsageResponse = response
            .json()
            .await
            .map_err(|error| WebSearchError::Usage(error.to_string()))?;
        Ok(WebSearchAccountUsage {
            current_plan: body.account.current_plan,
            plan_credits_used: body.account.plan_usage,
            paygo_credits_used: body.account.paygo_usage,
        })
    }
}

impl SearchResult {
    fn into_domain(self) -> WebSearchResult {
        let snippet = crate::markdown_links::absolutize_markdown_links(&self.content, &self.url);
        WebSearchResult {
            title: self.title,
            url: self.url,
            published_date: self.published_date,
            snippet,
        }
    }
}

#[derive(Serialize)]
struct SearchRequest<'a> {
    query: &'a str,
    search_depth: &'static str,
    max_results: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_range: Option<&'static str>,
    include_published_date: bool,
    include_usage: bool,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct SearchResult {
    title: String,
    url: String,
    content: String,
    #[serde(default)]
    published_date: Option<String>,
}

#[derive(Serialize)]
struct ExtractRequest<'a> {
    urls: [&'a str; 1],
    extract_depth: &'static str,
    format: &'static str,
    include_usage: bool,
}

#[derive(Deserialize)]
struct ExtractResponse {
    #[serde(default)]
    results: Vec<ExtractResult>,
    #[serde(default)]
    failed_results: Vec<ExtractFailure>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    credits: Decimal,
}

#[derive(Deserialize)]
struct AccountUsageResponse {
    account: AccountUsage,
}

#[derive(Deserialize)]
struct AccountUsage {
    current_plan: String,
    plan_usage: Decimal,
    paygo_usage: Decimal,
}

#[derive(Deserialize)]
struct ExtractResult {
    raw_content: String,
}

#[derive(Deserialize)]
struct ExtractFailure {
    url: String,
    error: String,
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    struct UsageMockServer(MockServer);

    impl UsageMockServer {
        async fn client_with_response(&self, body: serde_json::Value) -> TavilyClient {
            Mock::given(method("GET"))
                .and(path("/usage"))
                .and(header("authorization", "Bearer fake-tavily-key"))
                .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(body))
                .mount(&self.0)
                .await;

            TavilyClient::with_endpoints(
                format!("{}/search", self.0.uri()),
                format!("{}/extract", self.0.uri()),
                format!("{}/usage", self.0.uri()),
                "fake-tavily-key",
            )
            .expect("build client")
        }
    }

    #[fixture]
    async fn usage_mock_server() -> UsageMockServer {
        UsageMockServer(MockServer::start().await)
    }

    #[test]
    fn from_env_returns_none_when_api_key_is_missing() {
        assert!(TavilyClient::from_env_with(|_| None).is_none());
    }

    #[tokio::test]
    async fn search_sends_fixed_options_and_maps_result_fields() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .and(header("authorization", "Bearer fake-tavily-key"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "results": [{
                    "title": "Example article",
                    "url": "https://news.example.com/reports/section/overview",
                    "content": "Intro [root](/reports/annual), [relative](./detail), [parent](../archive), [external](https://docs.example.org/guide), [escaped](/reports/annual\\_summary), [titled](./detail \"section title\"), and code `[literal](/literal)`. ",
                    "raw_content": "# Article body",
                    "published_date": "2026-01-02"
                }, {
                    "title": "Invalid address",
                    "url": "invalid address",
                    "content": "[relative](/reports/annual) remains unchanged."
                }],
                "usage": { "credits": 1 }
            })))
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
            format!("{}/usage", mock.uri()),
            "fake-tavily-key",
        )
        .expect("build client");
        let result = client
            .search(
                "example query",
                Some(WebSearchTopic::News),
                Some(WebSearchTimeRange::Week),
            )
            .await
            .expect("search Tavily");

        let requests = mock.received_requests().await.expect("recorded requests");
        let request_body: serde_json::Value = requests[0].body_json().expect("parse request body");
        assert_eq!(
            (request_body, result),
            (
                json!({
                    "query": "example query",
                    "search_depth": "basic",
                    "max_results": 5,
                    "topic": "news",
                    "time_range": "week",
                    "include_published_date": true,
                    "include_usage": true
                }),
                WebSearchResponse {
                    results: vec![
                    WebSearchResult {
                        title: "Example article".into(),
                        url: "https://news.example.com/reports/section/overview".into(),
                        published_date: Some("2026-01-02".into()),
                        snippet: "Intro [root](https://news.example.com/reports/annual), [relative](https://news.example.com/reports/section/detail), [parent](https://news.example.com/reports/archive), [external](https://docs.example.org/guide), [escaped](https://news.example.com/reports/annual_summary), [titled](https://news.example.com/reports/section/detail \"section title\"), and code `[literal](/literal)`. ".into(),
                    },
                    WebSearchResult {
                        title: "Invalid address".into(),
                        url: "invalid address".into(),
                        published_date: None,
                        snippet: "[relative](/reports/annual) remains unchanged.".into(),
                    },
                    ],
                    credits_used: Some(Decimal::ONE),
                },
            ),
        );
    }

    #[tokio::test]
    async fn search_maps_http_errors() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(
                ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .set_body_string("upstream is temporarily unavailable"),
            )
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
            format!("{}/usage", mock.uri()),
            "fake-tavily-key",
        )
        .expect("build client");
        assert_eq!(
            client.search("example query", None, None).await,
            Err(WebSearchError::Api {
                status: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                message: "upstream is temporarily unavailable".into(),
            }),
        );
    }

    #[tokio::test]
    async fn extract_page_maps_http_errors() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(
                ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .set_body_string("upstream is temporarily unavailable"),
            )
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
            format!("{}/usage", mock.uri()),
            "fake-tavily-key",
        )
        .expect("build client");
        assert_eq!(
            client.extract_page("https://example.invalid/article").await,
            Err(WebSearchError::Api {
                status: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                message: "upstream is temporarily unavailable".into(),
            }),
        );
    }

    #[tokio::test]
    async fn extract_page_sends_basic_markdown_request_and_returns_full_content() {
        let mock = MockServer::start().await;
        let raw_content = "あ".repeat(100_001);
        Mock::given(method("POST"))
            .and(path("/extract"))
            .and(header("authorization", "Bearer fake-tavily-key"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "results": [{
                    "url": "https://example.invalid/article",
                    "raw_content": raw_content
                }],
                "failed_results": [],
                "usage": { "credits": 1 }
            })))
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
            format!("{}/usage", mock.uri()),
            "fake-tavily-key",
        )
        .expect("build client");
        let result = client
            .extract_page("https://example.invalid/article")
            .await
            .expect("extract page");
        let requests = mock.received_requests().await.expect("recorded requests");
        let request_body: serde_json::Value = requests[0].body_json().expect("parse request body");

        assert_eq!(
            (request_body, result),
            (
                json!({
                    "urls": ["https://example.invalid/article"],
                    "extract_depth": "basic",
                    "format": "markdown",
                    "include_usage": true
                }),
                WebPageExtractionResponse {
                    raw_content: Some("あ".repeat(100_001)),
                    failure: None,
                    credits_used: Some(Decimal::ONE),
                },
            ),
        );
    }

    #[tokio::test]
    async fn extract_page_returns_failed_result_reason() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "results": [],
                "failed_results": [{
                    "url": "https://example.invalid/article",
                    "error": "page requires a subscription"
                }],
                "usage": { "credits": 0 }
            })))
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
            format!("{}/usage", mock.uri()),
            "fake-tavily-key",
        )
        .expect("build client");

        assert_eq!(
            client.extract_page("https://example.invalid/article").await,
            Ok(WebPageExtractionResponse {
                raw_content: None,
                failure: Some(WebPageExtractionFailure {
                    url: "https://example.invalid/article".into(),
                    message: "page requires a subscription".into(),
                }),
                credits_used: Some(Decimal::ZERO),
            }),
        );
    }

    #[rstest]
    #[case::account_totals_ignore_key_usage(
        json!({
            "key": { "usage": 900.5 },
            "account": {
                "current_plan": "Example Plan",
                "plan_usage": 12.375,
                "paygo_usage": 0.125
            }
        }),
        WebSearchAccountUsage {
            current_plan: "Example Plan".into(),
            plan_credits_used: Decimal::new(12_375, 3),
            paygo_credits_used: Decimal::new(125, 3),
        },
    )]
    #[case::integer_credits_remain_exact(
        json!({
            "key": { "usage": 700 },
            "account": {
                "current_plan": "Example Plan",
                "plan_usage": 24,
                "paygo_usage": 0
            }
        }),
        WebSearchAccountUsage {
            current_plan: "Example Plan".into(),
            plan_credits_used: Decimal::new(24, 0),
            paygo_credits_used: Decimal::ZERO,
        },
    )]
    #[tokio::test]
    async fn account_usage_reads_account_totals(
        #[future] usage_mock_server: UsageMockServer,
        #[case] body: serde_json::Value,
        #[case] expected: WebSearchAccountUsage,
    ) {
        let usage_mock_server = usage_mock_server.await;
        let client = usage_mock_server.client_with_response(body).await;

        assert_eq!(client.account_usage().await, Ok(expected));
    }

    #[rstest]
    #[case::missing_account(json!({ "key": { "usage": 7 } }))]
    #[case::legacy_top_level_shape(json!({
        "current_plan": "Example Plan",
        "plan_usage": { "credits_used": 7 },
        "paygo_usage": { "credits_used": 0 }
    }))]
    #[case::missing_plan_usage(json!({
        "account": { "current_plan": "Example Plan", "paygo_usage": 0 }
    }))]
    #[case::missing_paygo_usage(json!({
        "account": { "current_plan": "Example Plan", "plan_usage": 7 }
    }))]
    #[case::missing_current_plan(json!({
        "account": { "plan_usage": 7, "paygo_usage": 0 }
    }))]
    #[case::invalid_account_shape(json!({ "account": [] }))]
    #[case::invalid_current_plan_type(json!({
        "account": { "current_plan": 7, "plan_usage": 7, "paygo_usage": 0 }
    }))]
    #[case::invalid_plan_usage_type(json!({
        "account": { "current_plan": "Example Plan", "plan_usage": {}, "paygo_usage": 0 }
    }))]
    #[case::invalid_paygo_usage_type(json!({
        "account": {
            "current_plan": "Example Plan",
            "plan_usage": 7,
            "paygo_usage": "not-a-number"
        }
    }))]
    #[tokio::test]
    async fn account_usage_rejects_missing_or_invalid_account_fields(
        #[future] usage_mock_server: UsageMockServer,
        #[case] body: serde_json::Value,
    ) {
        let usage_mock_server = usage_mock_server.await;
        let client = usage_mock_server.client_with_response(body).await;

        let actual = match client.account_usage().await {
            Ok(_) => "success",
            Err(WebSearchError::Usage(_)) => "usage error",
            Err(_) => "other error",
        };

        assert_eq!(actual, "usage error");
    }
}
