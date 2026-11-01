use std::time::Duration;

use async_trait::async_trait;
use rate_limit::{Quota, RateLimiter};
use reqwest::{Client, StatusCode, Url, redirect::Policy};

const OFFICIAL_ORIGIN: &str = "https://www.ecb.europa.eu";
const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_MAX_WAIT: Duration = Duration::from_secs(60);
const MAX_REDIRECTS: usize = 5;

#[async_trait]
pub(super) trait OfficialHttp: Send + Sync {
    async fn get_text(&self, url: &str) -> Result<String, String>;
}

#[derive(Clone)]
pub(super) struct OfficialHttpClient {
    http: Client,
    rate_limiter: RateLimiter,
}

impl OfficialHttpClient {
    pub(super) fn new(redis_url: &str) -> Result<Self, String> {
        let rate_limiter = RateLimiter::new(redis_url, RATE_LIMIT_KEY_PREFIX).map_err(|error| {
            format!("failed to initialize official source rate limiter: {error}")
        })?;
        let http = Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .user_agent("t-rader/0.1 (central bank archive lookup)")
            .build()
            .map_err(|error| format!("failed to initialize official HTTP client: {error}"))?;
        Ok(Self { http, rate_limiter })
    }

    pub(super) async fn get_text(&self, url: &str) -> Result<String, String> {
        let mut current = Url::parse(url).map_err(|error| error.to_string())?;
        validate_url(&current)?;

        for _ in 0..=MAX_REDIRECTS {
            let host = current
                .host_str()
                .ok_or_else(|| "official URL has no host".to_owned())?;
            self.rate_limiter
                .acquire(
                    &[Quota::new(
                        format!("official-source:{host}"),
                        1,
                        Duration::from_secs(1),
                    )],
                    RATE_LIMIT_MAX_WAIT,
                )
                .await
                .map_err(|error| format!("official source rate limit failed: {error}"))?;

            let response = self
                .http
                .get(current.clone())
                .send()
                .await
                .map_err(|error| error.without_url().to_string())?;
            let status = response.status();
            if is_redirect(status) {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| "official redirect has no valid location".to_owned())?;
                current = current
                    .join(location)
                    .map_err(|error| format!("invalid official redirect: {error}"))?;
                validate_url(&current)?;
                continue;
            }
            if !status.is_success() {
                return Err(format!("official source returned HTTP {status}"));
            }
            return response
                .text()
                .await
                .map_err(|error| error.without_url().to_string());
        }

        Err("official source redirected too many times".to_owned())
    }

    pub(super) fn resolve_url(base: &str, href: &str) -> Option<String> {
        let base = Url::parse(base).ok()?;
        validate_url(&base).ok()?;
        let url = base.join(href).ok()?;
        validate_url(&url).ok()?;
        Some(url.to_string())
    }
}

#[async_trait]
impl OfficialHttp for OfficialHttpClient {
    async fn get_text(&self, url: &str) -> Result<String, String> {
        OfficialHttpClient::get_text(self, url).await
    }
}

fn validate_url(url: &Url) -> Result<(), String> {
    if url.origin().ascii_serialization() == OFFICIAL_ORIGIN
        && url.username().is_empty()
        && url.password().is_none()
    {
        Ok(())
    } else {
        Err("URL is outside the official institution origin".to_owned())
    }
}

fn is_redirect(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    )
}

#[cfg(test)]
mod tests {
    use super::OfficialHttpClient;

    #[test]
    fn resolves_only_https_links_from_the_official_origin() {
        assert_eq!(
            OfficialHttpClient::resolve_url(
                "https://www.ecb.europa.eu/archive/index.html",
                "/documents/release.html"
            ),
            Some("https://www.ecb.europa.eu/documents/release.html".to_owned()),
        );
        assert_eq!(
            OfficialHttpClient::resolve_url(
                "https://www.ecb.europa.eu/archive/index.html",
                "http://www.example.invalid/release.html"
            ),
            None,
        );
    }
}
