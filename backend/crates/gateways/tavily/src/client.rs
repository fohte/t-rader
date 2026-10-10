use std::time::Duration;

use async_trait::async_trait;
use core_application::web_search::{
    WebSearchClient, WebSearchError, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

const SEARCH_ENDPOINT: &str = "https://api.tavily.com/search";
const EXTRACT_ENDPOINT: &str = "https://api.tavily.com/extract";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct TavilyClient {
    http: Client,
    search_endpoint: String,
    extract_endpoint: String,
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
        })
    }
}

#[async_trait]
impl WebSearchClient for TavilyClient {
    async fn search(
        &self,
        query: &str,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        let response = self
            .http
            .post(&self.search_endpoint)
            .json(&SearchRequest {
                query,
                search_depth: "basic",
                max_results: 5,
                topic: topic.map(WebSearchTopic::as_str),
                time_range: time_range.map(WebSearchTimeRange::as_str),
                include_published_date: true,
            })
            .send()
            .await
            .map_err(|error| WebSearchError::Network(error.to_string()))?;

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
        Ok(body
            .results
            .into_iter()
            .map(SearchResult::into_domain)
            .collect())
    }

    async fn extract_page(&self, url: &str) -> Result<String, WebSearchError> {
        let response = self
            .http
            .post(&self.extract_endpoint)
            .json(&ExtractRequest {
                urls: [url],
                extract_depth: "basic",
                format: "markdown",
            })
            .send()
            .await
            .map_err(|error| WebSearchError::Network(error.to_string()))?;

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
        if let Some(result) = body.results.into_iter().next() {
            return Ok(result.raw_content);
        }
        if let Some(failure) = body.failed_results.into_iter().next() {
            return Err(WebSearchError::ExtractionFailed {
                url: failure.url,
                message: failure.error,
            });
        }

        Err(WebSearchError::Parse(
            "Tavily extract response contains no results".to_string(),
        ))
    }
}

impl SearchResult {
    fn into_domain(self) -> WebSearchResult {
        WebSearchResult {
            title: self.title,
            url: self.url,
            published_date: self.published_date,
            snippet: self.content,
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
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
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
}

#[derive(Deserialize)]
struct ExtractResponse {
    #[serde(default)]
    results: Vec<ExtractResult>,
    #[serde(default)]
    failed_results: Vec<ExtractFailure>,
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
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

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
                    "url": "https://example.invalid/article",
                    "content": "A short search snippet",
                    "raw_content": "# Article body",
                    "published_date": "2026-01-02"
                }]
            })))
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
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
                    "include_published_date": true
                }),
                vec![WebSearchResult {
                    title: "Example article".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: Some("2026-01-02".into()),
                    snippet: "A short search snippet".into(),
                }],
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
                "failed_results": []
            })))
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
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
                    "format": "markdown"
                }),
                "あ".repeat(100_001),
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
                }]
            })))
            .mount(&mock)
            .await;

        let client = TavilyClient::with_endpoints(
            format!("{}/search", mock.uri()),
            format!("{}/extract", mock.uri()),
            "fake-tavily-key",
        )
        .expect("build client");

        assert_eq!(
            client.extract_page("https://example.invalid/article").await,
            Err(WebSearchError::ExtractionFailed {
                url: "https://example.invalid/article".into(),
                message: "page requires a subscription".into(),
            }),
        );
    }
}
