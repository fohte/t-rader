use std::time::Duration;

use core_application::news_aggregator::NewsAggregatorError;
use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{
    Response, Url,
    header::{LOCATION, RETRY_AFTER},
};

const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_PERIOD: Duration = Duration::from_secs(10);
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(60);
const RATE_LIMIT_LIMIT: u32 = 1;
const RATE_LIMIT_FALLBACK_COOLDOWN: Duration = Duration::from_secs(60 * 60);
const RATE_LIMIT_MAX_COOLDOWN: Duration = Duration::from_secs(24 * 60 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_REDIRECTS: usize = 10;
// acquire の 60 秒、HTTP の 15 秒、penalty の Redis 操作時間を 1 feed の上限にする。
const REQUEST_TOTAL_TIMEOUT: Duration = Duration::from_secs(80);

enum RssRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    TestNoop,
}

impl RssRateLimiter {
    async fn acquire(&self, quota: &Quota) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => {
                rate_limiter
                    .acquire(std::slice::from_ref(quota), RATE_LIMIT_MAX_WAIT)
                    .await
            }
            #[cfg(test)]
            Self::TestNoop => Ok(()),
        }
    }

    async fn penalize(&self, key: &str, duration: Duration) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => rate_limiter.penalize(key, duration).await,
            #[cfg(test)]
            Self::TestNoop => Ok(()),
        }
    }
}

pub(super) struct RssHttpClient {
    client: reqwest::Client,
    rate_limiter: RssRateLimiter,
}

impl RssHttpClient {
    pub(super) fn new(redis_url: &str) -> Result<Self, NewsAggregatorError> {
        Self::with_key_prefix(redis_url, RATE_LIMIT_KEY_PREFIX)
    }

    fn with_key_prefix(
        redis_url: &str,
        key_prefix: impl Into<String>,
    ) -> Result<Self, NewsAggregatorError> {
        let client = build_http_client()?;
        let rate_limiter = RateLimiter::new(redis_url, key_prefix).map_err(map_rate_limit_error)?;

        Ok(Self {
            client,
            rate_limiter: RssRateLimiter::Shared(rate_limiter),
        })
    }

    #[cfg(test)]
    pub(super) fn without_rate_limiter() -> Result<Self, NewsAggregatorError> {
        Ok(Self {
            client: build_http_client()?,
            rate_limiter: RssRateLimiter::TestNoop,
        })
    }

    pub(super) async fn send(&self, url: &Url) -> Result<Response, NewsAggregatorError> {
        let mut current_url = url.clone();
        let mut redirects_followed = 0;
        let deadline = tokio::time::Instant::now() + REQUEST_TOTAL_TIMEOUT;

        loop {
            let key = host_key(&current_url)?;
            let quota = Quota::new(key.clone(), RATE_LIMIT_LIMIT, RATE_LIMIT_PERIOD);
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(request_timeout_error());
            }
            let acquire_started = std::time::Instant::now();
            let acquire_result =
                tokio::time::timeout(remaining, self.rate_limiter.acquire(&quota)).await;
            let acquire_duration = acquire_started.elapsed();
            if acquire_duration >= Duration::from_millis(100) {
                tracing::info!(
                    wait_ms = acquire_duration.as_millis() as u64,
                    "RSS の共有 rate limit 取得に時間がかかりました"
                );
            }
            match acquire_result {
                Ok(result) => result.map_err(map_rate_limit_error)?,
                Err(_) => return Err(request_timeout_error()),
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(request_timeout_error());
            }
            let response =
                tokio::time::timeout(remaining, self.client.get(current_url.clone()).send())
                    .await
                    .map_err(|_| request_timeout_error())?
                    .map_err(|error| NewsAggregatorError::Network(error.to_string()))?;

            if matches!(response.status().as_u16(), 429 | 503) {
                let cooldown = retry_after_duration(
                    response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|value| value.to_str().ok()),
                );
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    return Err(request_timeout_error());
                }
                tokio::time::timeout(remaining, self.rate_limiter.penalize(&key, cooldown))
                    .await
                    .map_err(|_| request_timeout_error())?
                    .map_err(map_rate_limit_error)?;
            }

            if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                return Ok(response);
            }
            if redirects_followed == MAX_REDIRECTS {
                return Err(NewsAggregatorError::Network(
                    "RSS feed exceeded the redirect limit".to_owned(),
                ));
            }
            let Some(location) = response.headers().get(LOCATION) else {
                return Ok(response);
            };
            let location = location.to_str().map_err(|error| {
                NewsAggregatorError::Network(format!("invalid RSS redirect location: {error}"))
            })?;
            current_url = current_url.join(location).map_err(|error| {
                NewsAggregatorError::Network(format!("invalid RSS redirect URL: {error}"))
            })?;
            if !matches!(current_url.scheme(), "http" | "https") {
                return Err(NewsAggregatorError::Network(
                    "RSS redirect uses an unsupported URL scheme".to_owned(),
                ));
            }
            redirects_followed += 1;
        }
    }
}

fn request_timeout_error() -> NewsAggregatorError {
    NewsAggregatorError::Network("RSS feed fetch timed out".to_owned())
}

fn build_http_client() -> Result<reqwest::Client, NewsAggregatorError> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent("t-rader/0.1 (news aggregator)")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| NewsAggregatorError::Initialization(error.to_string()))
}

fn host_key(url: &Url) -> Result<String, NewsAggregatorError> {
    let host = url
        .host_str()
        .ok_or_else(|| NewsAggregatorError::Parse("invalid feed URL: missing host".to_owned()))?;

    Ok(format!("rss:host:{}", host.to_ascii_lowercase()))
}

fn retry_after_duration(value: Option<&str>) -> Duration {
    value
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds).min(RATE_LIMIT_MAX_COOLDOWN))
        .unwrap_or(RATE_LIMIT_FALLBACK_COOLDOWN)
}

fn map_rate_limit_error(error: RateLimitError) -> NewsAggregatorError {
    NewsAggregatorError::Network(format!("RSS rate limit error: {error}"))
}

#[cfg(test)]
fn test_key_prefix() -> String {
    static NEXT_TEST_PREFIX: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    format!(
        "t-rader:ratelimit:test:{}:{timestamp}:{}:",
        std::process::id(),
        NEXT_TEST_PREFIX.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use rstest::rstest;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[rstest]
    #[case::lowercase_host(
        "https://feeds.example.invalid/rss.xml",
        "rss:host:feeds.example.invalid"
    )]
    #[case::host_is_not_the_parent_domain(
        "https://news.example.invalid/rss.xml",
        "rss:host:news.example.invalid"
    )]
    #[case::port_is_not_part_of_host(
        "https://feeds.example.invalid:8443/rss.xml",
        "rss:host:feeds.example.invalid"
    )]
    fn creates_a_quota_key_from_the_feed_host(#[case] url: &str, #[case] expected: &str) {
        let url = Url::parse(url).expect("URL parses");

        assert_eq!(host_key(&url).as_deref(), Ok(expected));
    }

    #[rstest]
    #[case::retry_after_seconds(Some("12"), Duration::from_secs(12))]
    #[case::retry_after_is_trimmed(Some(" 12 "), Duration::from_secs(12))]
    #[case::zero(Some("0"), Duration::ZERO)]
    #[case::capped_at_twenty_four_hours(Some("90000"), Duration::from_secs(24 * 60 * 60))]
    #[case::missing(None, Duration::from_secs(60 * 60))]
    #[case::invalid(Some("later"), Duration::from_secs(60 * 60))]
    fn parses_retry_after(#[case] value: Option<&str>, #[case] expected: Duration) {
        assert_eq!(retry_after_duration(value), expected);
    }

    #[tokio::test]
    async fn send_rejects_redirects_to_non_http_schemes() -> Result<(), Box<dyn Error>> {
        let client = RssHttpClient::without_rate_limiter()?;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/redirect"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("Location", "file:///fictional/feed.xml"),
            )
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/redirect", server.uri()))?;
        let result = client.send(&url).await.map(|_| ());
        let requests = server
            .received_requests()
            .await
            .map(|requests| requests.len());

        assert_eq!(
            (result, requests),
            (
                Err(NewsAggregatorError::Network(
                    "RSS redirect uses an unsupported URL scheme".to_owned(),
                )),
                Some(1),
            ),
        );
        Ok(())
    }

    #[tokio::test]
    async fn send_stops_after_the_redirect_limit() -> Result<(), Box<dyn Error>> {
        let client = RssHttpClient::without_rate_limiter()?;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/redirect"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/redirect"))
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/redirect", server.uri()))?;
        let result = client.send(&url).await.map(|_| ());
        let requests = server
            .received_requests()
            .await
            .map(|requests| requests.len());

        assert_eq!(
            (result, requests),
            (
                Err(NewsAggregatorError::Network(
                    "RSS feed exceeded the redirect limit".to_owned(),
                )),
                Some(MAX_REDIRECTS + 1),
            ),
        );
        Ok(())
    }

    #[rstest]
    #[case::too_many_requests(429)]
    #[case::service_unavailable(503)]
    #[tokio::test]
    async fn status_penalties_are_shared_per_host_and_leave_other_hosts_available(
        #[case] status: u16,
    ) -> Result<(), Box<dyn Error>> {
        let redis_url = std::env::var("REDIS_URL")?;
        let key_prefix = test_key_prefix();
        let first = RssHttpClient::with_key_prefix(&redis_url, key_prefix.clone())?;
        let second = RssHttpClient::with_key_prefix(&redis_url, key_prefix)?;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/feed.xml", server.uri()))?;
        let response_status = first.send(&url).await?.status().as_u16();
        let quota = Quota::new(host_key(&url)?, RATE_LIMIT_LIMIT, RATE_LIMIT_PERIOD);
        let blocked = second
            .rate_limiter
            .acquire(&quota)
            .await
            .map_err(|error| error.to_string());
        let other_host = Quota::new(
            "rss:host:other.example.invalid",
            RATE_LIMIT_LIMIT,
            RATE_LIMIT_PERIOD,
        );
        let available = second
            .rate_limiter
            .acquire(&other_host)
            .await
            .map_err(|error| error.to_string());

        assert_eq!(
            (response_status, blocked, available),
            (
                status,
                Err(format!(
                    "rate limit acquisition exceeded the maximum wait of {:?}",
                    RATE_LIMIT_MAX_WAIT
                )),
                Ok(()),
            ),
        );
        Ok(())
    }
}
