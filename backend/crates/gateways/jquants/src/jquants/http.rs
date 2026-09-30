use std::time::Duration;

use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Response, Url};

use crate::{DataProviderError, JQuantsPlan};

const RATE_LIMIT_KEY_PREFIX: &str = "t-rader:ratelimit:";
const RATE_LIMIT_PERIOD: Duration = Duration::from_secs(60);
const RATE_LIMIT_SAFETY_FACTOR: usize = 2;
const FIN_SUMMARY_RATE_LIMIT_PER_MINUTE: usize = 60;
const FIN_SUMMARY_PATH: &str = "/fins/summary";
const GLOBAL_QUOTA_KEY: &str = "jquants:all";
const RATE_LIMIT_COOLDOWN_KEY: &str = "jquants:all";
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(5 * 60);

/// J-Quants API への送信と共有 quota の取得をまとめる。
pub(super) struct JQuantsHttpClient {
    client: reqwest::Client,
    rate_limiter: RateLimiter,
    plan: JQuantsPlan,
    max_wait: Duration,
}

impl JQuantsHttpClient {
    pub(super) fn new(
        redis_url: &str,
        plan: JQuantsPlan,
        max_wait: Duration,
    ) -> Result<Self, DataProviderError> {
        Self::with_key_prefix(
            redis_url,
            RATE_LIMIT_KEY_PREFIX,
            plan,
            max_wait,
            Duration::from_secs(30),
        )
    }

    pub(super) fn with_key_prefix(
        redis_url: &str,
        key_prefix: impl Into<String>,
        plan: JQuantsPlan,
        max_wait: Duration,
        request_timeout: Duration,
    ) -> Result<Self, DataProviderError> {
        let client = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()
            .map_err(|error| DataProviderError::Network(error.to_string()))?;
        let rate_limiter = RateLimiter::new(redis_url, key_prefix).map_err(map_rate_limit_error)?;

        Ok(Self {
            client,
            rate_limiter,
            plan,
            max_wait,
        })
    }

    pub(super) async fn send(
        &self,
        url: &Url,
        api_key: &str,
    ) -> Result<Response, DataProviderError> {
        let quotas = quotas_for_url(url, self.plan);
        self.rate_limiter
            .acquire(&quotas, self.max_wait)
            .await
            .map_err(map_rate_limit_error)?;

        let response = self
            .client
            .get(url.clone())
            .header("x-api-key", api_key)
            .send()
            .await
            .map_err(|error| DataProviderError::Network(error.to_string()))?;

        if response.status().as_u16() == 429 {
            let retry_after = retry_after_duration(
                response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok()),
            );
            self.rate_limiter
                .penalize(RATE_LIMIT_COOLDOWN_KEY, retry_after)
                .await
                .map_err(map_rate_limit_error)?;
        }

        Ok(response)
    }
}

fn quotas_for_url(url: &Url, plan: JQuantsPlan) -> Vec<Quota> {
    quota_limits_for_url(url, plan)
        .into_iter()
        .map(|(key, limit)| Quota::new(key, limit, RATE_LIMIT_PERIOD))
        .collect()
}

fn quota_limits_for_url(url: &Url, plan: JQuantsPlan) -> Vec<(&'static str, u32)> {
    let global_limit = (plan.rate_limit_per_minute() / RATE_LIMIT_SAFETY_FACTOR).max(1) as u32;
    let mut quotas = vec![(GLOBAL_QUOTA_KEY, global_limit)];

    if url.path().ends_with(FIN_SUMMARY_PATH) {
        let fin_summary_limit =
            (FIN_SUMMARY_RATE_LIMIT_PER_MINUTE / RATE_LIMIT_SAFETY_FACTOR).max(1) as u32;
        quotas.push(("jquants:fins-summary", fin_summary_limit));
    }

    quotas
}

fn retry_after_duration(value: Option<&str>) -> Duration {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(RATE_LIMIT_COOLDOWN)
}

fn map_rate_limit_error(error: RateLimitError) -> DataProviderError {
    match error {
        RateLimitError::MaxWaitExceeded { max_wait } => {
            DataProviderError::RateLimitWaitExceeded { max_wait }
        }
        error => DataProviderError::RateLimit(error.to_string()),
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(super) fn test_key_prefix() -> String {
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
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[rstest]
    #[case::plan_quota(
        "https://api.example.invalid/v2/equities/master",
        JQuantsPlan::Standard,
        vec![("jquants:all", 60)],
    )]
    #[case::fin_summary_uses_both_quotas(
        "https://api.example.invalid/v2/fins/summary",
        JQuantsPlan::Standard,
        vec![("jquants:all", 60), ("jquants:fins-summary", 30)],
    )]
    #[case::free_plan_quota(
        "https://api.example.invalid/v2/fins/summary",
        JQuantsPlan::Free,
        vec![("jquants:all", 2), ("jquants:fins-summary", 30)],
    )]
    fn selects_quotas_from_the_request_url(
        #[case] url: &str,
        #[case] plan: JQuantsPlan,
        #[case] expected: Vec<(&'static str, u32)>,
    ) -> Result<(), Box<dyn Error>> {
        let url = Url::parse(url)?;

        assert_eq!(quota_limits_for_url(&url, plan), expected);
        Ok(())
    }

    #[rstest]
    #[case::retry_after_seconds(Some("12"), Duration::from_secs(12))]
    #[case::retry_after_zero(Some("0"), Duration::ZERO)]
    #[case::missing(None, RATE_LIMIT_COOLDOWN)]
    #[case::invalid(Some("later"), RATE_LIMIT_COOLDOWN)]
    fn parses_retry_after(#[case] value: Option<&str>, #[case] expected: Duration) {
        assert_eq!(retry_after_duration(value), expected);
    }

    #[tokio::test]
    async fn a_429_response_blocks_another_client_sharing_the_redis_prefix()
    -> Result<(), Box<dyn Error>> {
        let redis_url = std::env::var("REDIS_URL")?;
        let key_prefix = test_key_prefix();
        let max_wait = Duration::from_millis(50);
        let first = JQuantsHttpClient::with_key_prefix(
            &redis_url,
            key_prefix.clone(),
            JQuantsPlan::Standard,
            Duration::ZERO,
            Duration::from_secs(5),
        )?;
        let second = JQuantsHttpClient::with_key_prefix(
            &redis_url,
            key_prefix,
            JQuantsPlan::Standard,
            max_wait,
            Duration::from_secs(5),
        )?;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "2")
                    .set_body_string("rate limited"),
            )
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/endpoint", server.uri()))?;
        second
            .rate_limiter
            .acquire(&quotas_for_url(&url, JQuantsPlan::Standard), Duration::ZERO)
            .await?;

        let first_status = first.send(&url, "fictional-api-key").await?.status();
        let second_result = second.send(&url, "fictional-api-key").await.map(|_| ());
        let received_requests = server
            .received_requests()
            .await
            .map(|requests| requests.len());

        assert_eq!(
            (first_status, second_result, received_requests,),
            (
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                Err(DataProviderError::RateLimitWaitExceeded { max_wait }),
                Some(1),
            ),
        );
        Ok(())
    }

    #[tokio::test]
    async fn send_returns_when_quota_wait_exceeds_the_configured_limit()
    -> Result<(), Box<dyn Error>> {
        let redis_url = std::env::var("REDIS_URL")?;
        let max_wait = Duration::from_millis(50);
        let client = JQuantsHttpClient::with_key_prefix(
            &redis_url,
            test_key_prefix(),
            JQuantsPlan::Free,
            max_wait,
            Duration::from_secs(5),
        )?;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/endpoint", server.uri()))?;
        client
            .rate_limiter
            .acquire(&quotas_for_url(&url, JQuantsPlan::Free), Duration::ZERO)
            .await?;

        let first_status = client.send(&url, "fictional-api-key").await?.status();
        let second_result = client.send(&url, "fictional-api-key").await.map(|_| ());
        let received_requests = server
            .received_requests()
            .await
            .map(|requests| requests.len());

        assert_eq!(
            (first_status, second_result, received_requests,),
            (
                reqwest::StatusCode::OK,
                Err(DataProviderError::RateLimitWaitExceeded { max_wait }),
                Some(1),
            ),
        );
        Ok(())
    }
}
