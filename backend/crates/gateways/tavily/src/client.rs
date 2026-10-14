use std::time::Duration;

use async_trait::async_trait;
use core_application::web_search::{
    WebSearchClient, WebSearchError, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
};
use reqwest::Client;
use reqwest::Url;
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
        let snippet = absolutize_markdown_links(&self.content, &self.url);
        WebSearchResult {
            title: self.title,
            url: self.url,
            published_date: self.published_date,
            snippet,
        }
    }
}

fn absolutize_markdown_links(snippet: &str, page_url: &str) -> String {
    let Ok(base_url) = Url::parse(page_url) else {
        return snippet.to_owned();
    };

    let mut result = String::with_capacity(snippet.len());
    let mut copied_until = 0;
    let mut search_from = 0;

    while let Some((target_start, target_end, link_end)) =
        next_markdown_link_target(snippet, search_from)
    {
        search_from = link_end;
        let target = &snippet[target_start..target_end];
        if Url::parse(target).is_ok() {
            continue;
        }
        let Ok(absolute_url) = base_url.join(target) else {
            continue;
        };

        result.push_str(&snippet[copied_until..target_start]);
        result.push_str(absolute_url.as_str());
        copied_until = target_end;
    }

    result.push_str(&snippet[copied_until..]);
    result
}

fn next_markdown_link_target(
    snippet: &str,
    mut search_from: usize,
) -> Option<(usize, usize, usize)> {
    let bytes = snippet.as_bytes();
    while search_from + 1 < bytes.len() {
        if bytes[search_from] == b'`' && !is_escaped(bytes, search_from) {
            let tick_count = count_run(bytes, search_from, b'`');
            if let Some(code_end) = find_code_span_end(bytes, search_from, tick_count) {
                search_from = code_end;
                continue;
            }
            search_from += tick_count;
            continue;
        }

        if bytes[search_from] == b']'
            && bytes[search_from + 1] == b'('
            && !is_escaped(bytes, search_from)
            && let Some((target_start, target_end, link_end)) =
                parse_markdown_link_target(bytes, search_from + 1)
        {
            return Some((target_start, target_end, link_end));
        }

        search_from += 1;
    }
    None
}

fn parse_markdown_link_target(bytes: &[u8], open_paren: usize) -> Option<(usize, usize, usize)> {
    let mut cursor = skip_whitespace(bytes, open_paren + 1);
    let (target_start, target_end) = if bytes.get(cursor) == Some(&b'<') {
        let target_start = cursor + 1;
        cursor = target_start;
        while cursor < bytes.len() && (bytes[cursor] != b'>' || is_escaped(bytes, cursor)) {
            cursor += 1;
        }
        if cursor == bytes.len() {
            return None;
        }
        let target_end = cursor;
        cursor += 1;
        (target_start, target_end)
    } else {
        let target_start = cursor;
        let mut nested_parentheses = 0;
        while cursor < bytes.len() {
            match bytes[cursor] {
                b'\\' => cursor = (cursor + 2).min(bytes.len()),
                b'(' => {
                    nested_parentheses += 1;
                    cursor += 1;
                }
                b')' if nested_parentheses == 0 => break,
                b')' => {
                    nested_parentheses -= 1;
                    cursor += 1;
                }
                byte if byte.is_ascii_whitespace() => break,
                _ => cursor += 1,
            }
        }
        (target_start, cursor)
    };

    let link_end = parse_markdown_link_suffix(bytes, cursor)?;
    Some((target_start, target_end, link_end))
}

fn parse_markdown_link_suffix(bytes: &[u8], cursor: usize) -> Option<usize> {
    let mut cursor = skip_whitespace(bytes, cursor);
    match *bytes.get(cursor)? {
        b')' => Some(cursor + 1),
        quote @ (b'"' | b'\'') => {
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == quote {
                    cursor = skip_whitespace(bytes, cursor + 1);
                    return (bytes.get(cursor) == Some(&b')')).then_some(cursor + 1);
                } else {
                    cursor += 1;
                }
            }
            None
        }
        b'(' => {
            let mut depth = 1;
            cursor += 1;
            while cursor < bytes.len() {
                match bytes[cursor] {
                    b'\\' => cursor = (cursor + 2).min(bytes.len()),
                    b'(' => {
                        depth += 1;
                        cursor += 1;
                    }
                    b')' => {
                        depth -= 1;
                        cursor += 1;
                        if depth == 0 {
                            cursor = skip_whitespace(bytes, cursor);
                            return (bytes.get(cursor) == Some(&b')')).then_some(cursor + 1);
                        }
                    }
                    _ => cursor += 1,
                }
            }
            None
        }
        _ => None,
    }
}

fn find_code_span_end(bytes: &[u8], start: usize, tick_count: usize) -> Option<usize> {
    let mut cursor = start + tick_count;
    while cursor < bytes.len() {
        if bytes[cursor] == b'`' && !is_escaped(bytes, cursor) {
            let run_length = count_run(bytes, cursor, b'`');
            if run_length == tick_count {
                return Some(cursor + run_length);
            }
            cursor += run_length;
        } else {
            cursor += 1;
        }
    }
    None
}

fn count_run(bytes: &[u8], start: usize, byte: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|&&candidate| candidate == byte)
        .count()
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        cursor -= 1;
    }
    (index - cursor) % 2 == 1
}

fn skip_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
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
                    "url": "https://news.example.com/reports/section/overview",
                    "content": "Intro [root](/reports/annual), [relative](./detail), [parent](../archive), [external](https://docs.example.org/guide), and code `[literal](/literal)`. ",
                    "raw_content": "# Article body",
                    "published_date": "2026-01-02"
                }, {
                    "title": "Invalid address",
                    "url": "invalid address",
                    "content": "[relative](/reports/annual) remains unchanged."
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
                vec![
                    WebSearchResult {
                        title: "Example article".into(),
                        url: "https://news.example.com/reports/section/overview".into(),
                        published_date: Some("2026-01-02".into()),
                        snippet: "Intro [root](https://news.example.com/reports/annual), [relative](https://news.example.com/reports/section/detail), [parent](https://news.example.com/reports/archive), [external](https://docs.example.org/guide), and code `[literal](/literal)`. ".into(),
                    },
                    WebSearchResult {
                        title: "Invalid address".into(),
                        url: "invalid address".into(),
                        published_date: None,
                        snippet: "[relative](/reports/annual) remains unchanged.".into(),
                    },
                ],
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
