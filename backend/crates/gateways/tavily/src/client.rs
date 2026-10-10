use std::time::Duration;

use async_trait::async_trait;
use core_application::web_search::{
    WebSearchClient, WebSearchError, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
};
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

const SEARCH_ENDPOINT: &str = "https://api.tavily.com/search";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BODY_CHARS: usize = 5_000;

#[derive(Clone)]
pub struct TavilyClient {
    http: Client,
    endpoint: String,
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
        match Self::with_endpoint(SEARCH_ENDPOINT.to_owned(), &api_key) {
            Ok(client) => Some(client),
            Err(error) => {
                tracing::warn!(error = %error, "failed to initialize Tavily client");
                None
            }
        }
    }

    fn with_endpoint(endpoint: String, api_key: &str) -> Result<Self, WebSearchError> {
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

        Ok(Self { http, endpoint })
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
            .post(&self.endpoint)
            .json(&SearchRequest {
                query,
                search_depth: "basic",
                max_results: 5,
                topic: topic.map(WebSearchTopic::as_str),
                time_range: time_range.map(WebSearchTimeRange::as_str),
                include_raw_content: "markdown",
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
}

impl SearchResult {
    fn into_domain(self) -> WebSearchResult {
        let (body, body_truncated) = self
            .raw_content
            .map(truncate_body)
            .map_or((None, false), |(body, truncated)| (Some(body), truncated));
        WebSearchResult {
            title: self.title,
            url: self.url,
            published_date: self.published_date,
            snippet: self.content,
            body,
            body_truncated,
        }
    }
}

fn truncate_body(body: String) -> (String, bool) {
    let mut chars = body.chars();
    let truncated_body = chars.by_ref().take(MAX_BODY_CHARS).collect();
    (truncated_body, chars.next().is_some())
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
    include_raw_content: &'static str,
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
    raw_content: Option<String>,
    #[serde(default)]
    published_date: Option<String>,
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

        let client =
            TavilyClient::with_endpoint(format!("{}/search", mock.uri()), "fake-tavily-key")
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
                    "include_raw_content": "markdown",
                    "include_published_date": true
                }),
                vec![WebSearchResult {
                    title: "Example article".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: Some("2026-01-02".into()),
                    snippet: "A short search snippet".into(),
                    body: Some("# Article body".into()),
                    body_truncated: false,
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

        let client =
            TavilyClient::with_endpoint(format!("{}/search", mock.uri()), "fake-tavily-key")
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
    async fn search_truncates_body_by_unicode_characters() {
        let mock = MockServer::start().await;
        let raw_content = "あ".repeat(MAX_BODY_CHARS + 1);
        Mock::given(method("POST"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "results": [{
                    "title": "Example article",
                    "url": "https://example.invalid/article",
                    "content": "Snippet",
                    "raw_content": raw_content
                }]
            })))
            .mount(&mock)
            .await;

        let client =
            TavilyClient::with_endpoint(format!("{}/search", mock.uri()), "fake-tavily-key")
                .expect("build client");
        let result = client
            .search("example query", None, None)
            .await
            .expect("search Tavily");

        assert_eq!(
            result,
            vec![WebSearchResult {
                title: "Example article".into(),
                url: "https://example.invalid/article".into(),
                published_date: None,
                snippet: "Snippet".into(),
                body: Some("あ".repeat(MAX_BODY_CHARS)),
                body_truncated: true,
            }],
        );
    }
}
