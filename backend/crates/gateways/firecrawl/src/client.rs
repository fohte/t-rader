use std::time::Duration;

use async_trait::async_trait;
use core_application::news_content::{
    NewsContentFetchError, NewsContentFetchOutcome, NewsContentFetcher, NewsContentInterruption,
};
use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::json;
use thiserror::Error;

const SCRAPE_ENDPOINT: &str = "https://api.firecrawl.dev/v2/scrape";
const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const HOST_RATE_LIMIT: u32 = 1;
const HOST_RATE_PERIOD: Duration = Duration::from_secs(10);
const FIRECRAWL_RATE_LIMIT: u32 = 5;
const FIRECRAWL_RATE_PERIOD: Duration = Duration::from_secs(60);
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(60);
const HOST_PENALTY: Duration = Duration::from_secs(60 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Debug, Error)]
pub enum FirecrawlClientError {
    #[error("failed to build Firecrawl HTTP client")]
    HttpClient,
    #[error("failed to initialize Firecrawl rate limiter")]
    RateLimiter,
}

#[derive(Clone)]
enum FirecrawlRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    Disabled,
}

pub struct FirecrawlClient {
    client: Client,
    endpoint: String,
    api_key: String,
    rate_limiter: FirecrawlRateLimiter,
}

impl FirecrawlClient {
    pub fn new(redis_url: &str, api_key: String) -> Result<Self, FirecrawlClientError> {
        let rate_limiter = RateLimiter::new(redis_url, RATE_LIMIT_KEY_PREFIX)
            .map_err(|_| FirecrawlClientError::RateLimiter)?;
        Self::with_components(
            SCRAPE_ENDPOINT.to_owned(),
            api_key,
            FirecrawlRateLimiter::Shared(rate_limiter),
            HTTP_TIMEOUT,
        )
    }

    fn with_components(
        endpoint: String,
        api_key: String,
        rate_limiter: FirecrawlRateLimiter,
        timeout: Duration,
    ) -> Result<Self, FirecrawlClientError> {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|_| FirecrawlClientError::HttpClient)?;

        Ok(Self {
            client,
            endpoint,
            api_key,
            rate_limiter,
        })
    }

    #[cfg(test)]
    fn without_rate_limiter(
        endpoint: String,
        api_key: String,
        timeout: Duration,
    ) -> Result<Self, FirecrawlClientError> {
        Self::with_components(endpoint, api_key, FirecrawlRateLimiter::Disabled, timeout)
    }

    async fn acquire(&self, host_key: &str) -> Result<bool, NewsContentFetchError> {
        let quotas = [
            Quota::new(host_key, HOST_RATE_LIMIT, HOST_RATE_PERIOD),
            Quota::new("firecrawl:api", FIRECRAWL_RATE_LIMIT, FIRECRAWL_RATE_PERIOD),
        ];
        match &self.rate_limiter {
            FirecrawlRateLimiter::Shared(rate_limiter) => {
                match rate_limiter.acquire(&quotas, RATE_LIMIT_MAX_WAIT).await {
                    Ok(()) => Ok(true),
                    Err(RateLimitError::MaxWaitExceeded { .. }) => Ok(false),
                    Err(_) => Err(NewsContentFetchError::RateLimiter),
                }
            }
            #[cfg(test)]
            FirecrawlRateLimiter::Disabled => Ok(true),
        }
    }

    async fn penalize_host(&self, host_key: &str) -> Result<(), NewsContentFetchError> {
        match &self.rate_limiter {
            FirecrawlRateLimiter::Shared(rate_limiter) => rate_limiter
                .penalize(host_key, HOST_PENALTY)
                .await
                .map_err(|_| NewsContentFetchError::RateLimiter),
            #[cfg(test)]
            FirecrawlRateLimiter::Disabled => Ok(()),
        }
    }

    async fn handle_response(
        &self,
        response: reqwest::Response,
        host_key: &str,
    ) -> Result<NewsContentFetchOutcome, NewsContentFetchError> {
        match response.status().as_u16() {
            200 => {
                let Ok(scrape_response) = response.json::<ScrapeResponse>().await else {
                    return Ok(NewsContentFetchOutcome::Retry);
                };
                if scrape_response.success == Some(false) {
                    return Ok(NewsContentFetchOutcome::Retry);
                }
                let Some(data) = scrape_response.data else {
                    return Ok(NewsContentFetchOutcome::Retry);
                };
                let Some(status_code) = data.metadata.and_then(|metadata| metadata.status_code)
                else {
                    return Ok(NewsContentFetchOutcome::Retry);
                };
                match status_code {
                    200..=299 => match data.markdown {
                        Some(markdown) if !markdown.trim().is_empty() => {
                            Ok(NewsContentFetchOutcome::Fetched(markdown))
                        }
                        _ => Ok(NewsContentFetchOutcome::Failed("empty".to_owned())),
                    },
                    401 | 403 | 404 | 410 => Ok(NewsContentFetchOutcome::Failed(format!(
                        "http_{status_code}"
                    ))),
                    429 | 503 => {
                        self.penalize_host(host_key).await?;
                        Ok(NewsContentFetchOutcome::Retry)
                    }
                    500..=599 => Ok(NewsContentFetchOutcome::Retry),
                    _ => Ok(NewsContentFetchOutcome::Retry),
                }
            }
            400 => Ok(NewsContentFetchOutcome::Failed("firecrawl_400".to_owned())),
            402 => Ok(NewsContentFetchOutcome::Abort(
                NewsContentInterruption::FirecrawlCreditsExhausted,
            )),
            429 => Ok(NewsContentFetchOutcome::Abort(
                NewsContentInterruption::FirecrawlRateLimited,
            )),
            500..=599 => Ok(NewsContentFetchOutcome::Retry),
            _ => Ok(NewsContentFetchOutcome::Retry),
        }
    }
}

#[async_trait]
impl NewsContentFetcher for FirecrawlClient {
    async fn fetch(&self, url: &str) -> Result<NewsContentFetchOutcome, NewsContentFetchError> {
        let article_url = match Url::parse(url) {
            Ok(url) => url,
            Err(_) => return Ok(NewsContentFetchOutcome::Failed("invalid_url".to_owned())),
        };
        let Some(host) = article_url.host_str() else {
            return Ok(NewsContentFetchOutcome::Failed("invalid_url".to_owned()));
        };
        let host_key = format!("rss:host:{}", host.to_ascii_lowercase());
        if !self.acquire(&host_key).await? {
            return Ok(NewsContentFetchOutcome::Retry);
        }

        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&json!({
                "url": article_url.as_str(),
                "formats": ["markdown"],
                "onlyMainContent": true,
                "timeout": 30_000,
                "maxAge": 0,
                "storeInCache": false,
                "parsers": [{ "type": "pdf", "mode": "fast", "maxPages": 10 }],
            }))
            .send()
            .await;
        match response {
            Ok(response) => self.handle_response(response, &host_key).await,
            Err(_) => Ok(NewsContentFetchOutcome::Retry),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ScrapeResponse {
    success: Option<bool>,
    data: Option<ScrapeData>,
}

#[derive(Debug, Deserialize)]
struct ScrapeData {
    markdown: Option<String>,
    metadata: Option<ScrapeMetadata>,
}

#[derive(Debug, Deserialize)]
struct ScrapeMetadata {
    #[serde(rename = "statusCode")]
    status_code: Option<u16>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use core_application::news_content::{
        NewsContentFetchOutcome, NewsContentFetcher, NewsContentInterruption,
    };
    use reqwest::header::AUTHORIZATION;
    use rstest::rstest;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::FirecrawlClient;

    #[rstest]
    #[case::markdown(200, Some(200), Some("full article"), NewsContentFetchOutcome::Fetched("full article".to_owned()))]
    #[case::empty_markdown(200, Some(200), Some("  \n"), NewsContentFetchOutcome::Failed("empty".to_owned()))]
    #[case::missing_markdown(200, Some(200), None, NewsContentFetchOutcome::Failed("empty".to_owned()))]
    #[case::origin_unauthorized(200, Some(401), None, NewsContentFetchOutcome::Failed("http_401".to_owned()))]
    #[case::origin_forbidden(200, Some(403), None, NewsContentFetchOutcome::Failed("http_403".to_owned()))]
    #[case::origin_not_found(200, Some(404), None, NewsContentFetchOutcome::Failed("http_404".to_owned()))]
    #[case::origin_gone(200, Some(410), None, NewsContentFetchOutcome::Failed("http_410".to_owned()))]
    #[case::origin_rate_limited(200, Some(429), None, NewsContentFetchOutcome::Retry)]
    #[case::origin_unavailable(200, Some(503), None, NewsContentFetchOutcome::Retry)]
    #[case::origin_server_error(200, Some(500), None, NewsContentFetchOutcome::Retry)]
    #[case::firecrawl_bad_request(400, None, None, NewsContentFetchOutcome::Failed("firecrawl_400".to_owned()))]
    #[case::credits_exhausted(
        402,
        None,
        None,
        NewsContentFetchOutcome::Abort(NewsContentInterruption::FirecrawlCreditsExhausted)
    )]
    #[case::firecrawl_rate_limited(
        429,
        None,
        None,
        NewsContentFetchOutcome::Abort(NewsContentInterruption::FirecrawlRateLimited)
    )]
    #[case::firecrawl_server_error(500, None, None, NewsContentFetchOutcome::Retry)]
    #[tokio::test]
    async fn fetch_maps_firecrawl_and_origin_responses(
        #[case] api_status: u16,
        #[case] origin_status: Option<u16>,
        #[case] markdown: Option<&str>,
        #[case] expected: NewsContentFetchOutcome,
    ) {
        let server = MockServer::start().await;
        let response_body = match origin_status {
            Some(status_code) => json!({
                "success": true,
                "data": {
                    "markdown": markdown,
                    "metadata": { "statusCode": status_code },
                },
            }),
            None => json!({ "error": "sample error" }),
        };
        Mock::given(method("POST"))
            .and(path("/v2/scrape"))
            .respond_with(ResponseTemplate::new(api_status).set_body_json(response_body))
            .mount(&server)
            .await;
        let client = FirecrawlClient::without_rate_limiter(
            format!("{}/v2/scrape", server.uri()),
            "fake-firecrawl-key".to_owned(),
            Duration::from_secs(1),
        )
        .expect("test client builds");

        let actual = client.fetch("https://example.invalid/article").await;

        assert_eq!(actual, Ok(expected));
    }

    #[tokio::test]
    async fn fetch_sends_the_required_scrape_options_and_bearer_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/scrape"))
            .and(header(AUTHORIZATION, "Bearer fake-firecrawl-key"))
            .and(body_json(json!({
                "url": "https://example.invalid/article",
                "formats": ["markdown"],
                "onlyMainContent": true,
                "timeout": 30_000,
                "maxAge": 0,
                "storeInCache": false,
                "parsers": [{ "type": "pdf", "mode": "fast", "maxPages": 10 }],
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "success": true,
                "data": { "markdown": "full article", "metadata": { "statusCode": 200 } },
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = FirecrawlClient::without_rate_limiter(
            format!("{}/v2/scrape", server.uri()),
            "fake-firecrawl-key".to_owned(),
            Duration::from_secs(1),
        )
        .expect("test client builds");

        let actual = client.fetch("https://example.invalid/article").await;

        assert_eq!(
            actual,
            Ok(NewsContentFetchOutcome::Fetched("full article".to_owned()))
        );
    }

    #[tokio::test]
    async fn fetch_treats_a_timeout_as_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/scrape"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "success": true,
                        "data": { "markdown": "full article", "metadata": { "statusCode": 200 } },
                    }))
                    .set_delay(Duration::from_millis(100)),
            )
            .mount(&server)
            .await;
        let client = FirecrawlClient::without_rate_limiter(
            format!("{}/v2/scrape", server.uri()),
            "fake-firecrawl-key".to_owned(),
            Duration::from_millis(10),
        )
        .expect("test client builds");

        let actual = client.fetch("https://example.invalid/article").await;

        assert_eq!(actual, Ok(NewsContentFetchOutcome::Retry));
    }
}
