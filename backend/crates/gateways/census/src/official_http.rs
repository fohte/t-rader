use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rate_limit::{Quota, RateLimiter};
use reqwest::{Client, StatusCode, Url, header::CONTENT_TYPE};
use thiserror::Error;

const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_PER_SECOND: u32 = 1;
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(60);
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum OfficialHttpError {
    #[error("official source URL is invalid: {0}")]
    InvalidUrl(String),
    #[error("shared rate limiter failed: {0}")]
    RateLimit(String),
    #[error("official source request failed: {0}")]
    Request(String),
    #[error("official source returned HTTP {0}")]
    Status(StatusCode),
    #[error("official source response could not be read: {0}")]
    Body(String),
    #[error("official source text is not UTF-8: {0}")]
    Text(String),
    #[error("official HTTP client could not be initialized: {0}")]
    Client(String),
    #[error("official HTTP cache lock is poisoned")]
    CacheLock,
}

#[derive(Clone)]
pub(super) struct OfficialHttpClient {
    http: Client,
    rate_limiter: SharedRateLimiter,
    cache: Arc<Mutex<HashMap<String, (Instant, CachedResponse)>>>,
    allowed_domain: String,
    allow_http_for_test: bool,
}

#[derive(Clone)]
struct CachedResponse {
    body: Vec<u8>,
    content_type: Option<String>,
}

#[derive(Clone)]
enum SharedRateLimiter {
    Redis(RateLimiter),
    #[cfg(test)]
    Noop,
}

impl SharedRateLimiter {
    async fn acquire(&self, host: &str) -> Result<(), OfficialHttpError> {
        match self {
            Self::Redis(rate_limiter) => rate_limiter
                .acquire(
                    &[Quota::new(
                        format!("official-source:{host}"),
                        RATE_LIMIT_PER_SECOND,
                        Duration::from_secs(1),
                    )],
                    RATE_LIMIT_MAX_WAIT,
                )
                .await
                .map_err(|error| OfficialHttpError::RateLimit(error.to_string())),
            #[cfg(test)]
            Self::Noop => Ok(()),
        }
    }
}

impl OfficialHttpClient {
    pub(super) fn new(redis_url: &str, allowed_domain: &str) -> Result<Self, OfficialHttpError> {
        let rate_limiter = RateLimiter::new(redis_url, RATE_LIMIT_KEY_PREFIX)
            .map_err(|error| OfficialHttpError::RateLimit(error.to_string()))?;
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(HTTP_TIMEOUT)
            .user_agent("Mozilla/5.0 (compatible; t-rader official archive lookup)")
            .build()
            .map_err(|error| OfficialHttpError::Client(error.to_string()))?;

        Ok(Self {
            http,
            rate_limiter: SharedRateLimiter::Redis(rate_limiter),
            cache: Arc::new(Mutex::new(HashMap::new())),
            allowed_domain: allowed_domain.to_owned(),
            allow_http_for_test: false,
        })
    }

    pub(super) async fn get_text_cached(&self, url: &str) -> Result<String, OfficialHttpError> {
        let response = self.get_cached_response(url).await?;
        let label = response
            .content_type
            .as_deref()
            .and_then(charset_from_content_type)
            .unwrap_or("utf-8");
        let encoding = encoding_rs::Encoding::for_label(label.as_bytes())
            .ok_or_else(|| OfficialHttpError::Text(format!("unsupported charset: {label}")))?;
        let (decoded, _, had_errors) = encoding.decode(&response.body);
        if had_errors {
            return Err(OfficialHttpError::Text(format!(
                "response could not be decoded as {label}"
            )));
        }
        Ok(decoded.into_owned())
    }

    pub(super) async fn get_bytes_cached(&self, url: &str) -> Result<Vec<u8>, OfficialHttpError> {
        self.get_cached_response(url)
            .await
            .map(|response| response.body)
    }

    async fn get_cached_response(&self, url: &str) -> Result<CachedResponse, OfficialHttpError> {
        let cached = self
            .cache
            .lock()
            .map_err(|_| OfficialHttpError::CacheLock)?
            .get(url)
            .filter(|(fetched_at, _)| fetched_at.elapsed() < CACHE_TTL)
            .map(|(_, body)| body.clone());
        if let Some(body) = cached {
            return Ok(body);
        }

        let response = self.get_response(url).await?;
        let fetched_at = Instant::now();
        let mut cache = self
            .cache
            .lock()
            .map_err(|_| OfficialHttpError::CacheLock)?;
        cache.retain(|_, (cached_at, _)| cached_at.elapsed() < CACHE_TTL);
        cache.insert(url.to_owned(), (fetched_at, response.clone()));
        Ok(response)
    }

    async fn get_response(&self, url: &str) -> Result<CachedResponse, OfficialHttpError> {
        let mut parsed_url =
            Url::parse(url).map_err(|error| OfficialHttpError::InvalidUrl(error.to_string()))?;
        if !is_allowed_url(&parsed_url, &self.allowed_domain, self.allow_http_for_test) {
            return Err(OfficialHttpError::InvalidUrl(
                "URL is outside the allowed official domain".to_owned(),
            ));
        }
        for redirect_count in 0..=MAX_REDIRECTS {
            let host = parsed_url
                .host_str()
                .ok_or_else(|| OfficialHttpError::InvalidUrl("URL has no host".to_owned()))?;
            self.rate_limiter.acquire(host).await?;
            let response = self
                .http
                .get(parsed_url.clone())
                .send()
                .await
                .map_err(|error| OfficialHttpError::Request(error.without_url().to_string()))?;
            let status = response.status();
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
                if redirect_count == MAX_REDIRECTS {
                    return Err(OfficialHttpError::Request(
                        "official source redirected too many times".to_owned(),
                    ));
                }
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| {
                        OfficialHttpError::Request(
                            "official source redirect has no valid location".to_owned(),
                        )
                    })?;
                parsed_url = parsed_url
                    .join(location)
                    .map_err(|error| OfficialHttpError::InvalidUrl(error.to_string()))?;
                if !is_allowed_url(&parsed_url, &self.allowed_domain, self.allow_http_for_test) {
                    return Err(OfficialHttpError::InvalidUrl(
                        "redirect destination is outside the allowed official domain".to_owned(),
                    ));
                }
                continue;
            }
            if !status.is_success() {
                return Err(OfficialHttpError::Status(status));
            }
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = response
                .bytes()
                .await
                .map(|body| body.to_vec())
                .map_err(|error| OfficialHttpError::Body(error.without_url().to_string()))?;
            return Ok(CachedResponse { body, content_type });
        }
        Err(OfficialHttpError::Request(
            "official source redirected too many times".to_owned(),
        ))
    }

    #[cfg(test)]
    pub(super) fn for_test() -> Result<Self, OfficialHttpError> {
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| OfficialHttpError::Client(error.to_string()))?;
        Ok(Self {
            http,
            rate_limiter: SharedRateLimiter::Noop,
            cache: Arc::new(Mutex::new(HashMap::new())),
            allowed_domain: "127.0.0.1".to_owned(),
            allow_http_for_test: true,
        })
    }
}

fn is_allowed_url(url: &Url, allowed_domain: &str, allow_http_for_test: bool) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let allowed_domain = allowed_domain.trim_end_matches('.').to_ascii_lowercase();
    let allowed_host = host == allowed_domain
        || host
            .strip_suffix(&allowed_domain)
            .is_some_and(|prefix| prefix.ends_with('.'));
    let allowed_scheme = (url.scheme() == "https" && url.port_or_known_default() == Some(443))
        || (allow_http_for_test && url.scheme() == "http" && host == "127.0.0.1");

    allowed_host && allowed_scheme && url.username().is_empty() && url.password().is_none()
}

fn charset_from_content_type(content_type: &str) -> Option<&str> {
    content_type.split(';').find_map(|parameter| {
        let (key, value) = parameter.trim().split_once('=')?;
        key.trim().eq_ignore_ascii_case("charset").then(|| {
            value
                .trim()
                .trim_matches(|character| character == '\"' || character == '\'')
        })
    })
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

    use super::{OfficialHttpClient, OfficialHttpError};

    #[tokio::test]
    async fn not_found_status_is_returned_as_an_http_error() -> Result<(), OfficialHttpError> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let actual = OfficialHttpClient::for_test()?
            .get_text_cached(&server.uri())
            .await;

        assert_eq!(
            actual,
            Err(OfficialHttpError::Status(StatusCode::NOT_FOUND)),
        );
        Ok(())
    }

    #[tokio::test]
    async fn redirect_to_a_different_domain_is_rejected() -> Result<(), OfficialHttpError> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://example.invalid/redirect-target"),
            )
            .mount(&server)
            .await;

        let actual = OfficialHttpClient::for_test()?
            .get_text_cached(&server.uri())
            .await;

        assert_eq!(
            actual,
            Err(OfficialHttpError::InvalidUrl(
                "redirect destination is outside the allowed official domain".to_owned(),
            )),
        );
        Ok(())
    }

    #[tokio::test]
    async fn request_to_a_different_domain_is_rejected() -> Result<(), OfficialHttpError> {
        let actual = OfficialHttpClient::for_test()?
            .get_text_cached("https://example.invalid/archive")
            .await;

        assert_eq!(
            actual,
            Err(OfficialHttpError::InvalidUrl(
                "URL is outside the allowed official domain".to_owned(),
            )),
        );
        Ok(())
    }

    #[tokio::test]
    async fn transport_failure_is_returned_as_an_http_error() -> Result<(), OfficialHttpError> {
        let server = MockServer::start().await;
        let url = server.uri();
        drop(server);

        let actual = OfficialHttpClient::for_test()?
            .get_text_cached(&url)
            .await
            .map(|_| "success")
            .unwrap_or("failure");

        assert_eq!(actual, "failure");
        Ok(())
    }
}
