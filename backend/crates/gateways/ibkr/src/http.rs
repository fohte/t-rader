use std::time::Duration;

use rate_limit::{Quota, RateLimitError, RateLimiter};
use reqwest::{Response, Url};

use crate::IbkrError;

const RATE_LIMIT_GLOBAL_KEY: &str = "ibkr:all";
const RATE_LIMIT_HISTORY_SECOND_KEY: &str = "ibkr:history:second";
const RATE_LIMIT_HISTORY_MINUTE_KEY: &str = "ibkr:history:minute";
const RATE_LIMIT_PERIOD: Duration = Duration::from_secs(1);
const RATE_LIMIT_GLOBAL_PER_SECOND: u32 = 5;
const RATE_LIMIT_HISTORY_PER_SECOND: u32 = 5;
const RATE_LIMIT_HISTORY_PER_MINUTE: u32 = 25;
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(10 * 60);

enum IbkrRateLimiter {
    Shared(RateLimiter),
    #[cfg(test)]
    TestNoop,
}

impl IbkrRateLimiter {
    async fn acquire(&self, quotas: &[Quota], max_wait: Duration) -> Result<(), RateLimitError> {
        match self {
            Self::Shared(rate_limiter) => rate_limiter.acquire(quotas, max_wait).await,
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

/// IBKR への HTTP 送信と、全プロセスで共有する quota をまとめる。
pub(super) struct IbkrHttpClient {
    client: reqwest::Client,
    rate_limiter: IbkrRateLimiter,
    max_wait: Duration,
}

impl IbkrHttpClient {
    pub(super) fn new(rate_limiter: RateLimiter, max_wait: Duration) -> Result<Self, IbkrError> {
        Self::with_rate_limiter(rate_limiter, max_wait, Duration::from_secs(30))
    }

    fn with_rate_limiter(
        rate_limiter: RateLimiter,
        max_wait: Duration,
        request_timeout: Duration,
    ) -> Result<Self, IbkrError> {
        let client = reqwest::Client::builder()
            // CP Gateway は自己署名証明書を使うことが多いため、証明書検証は緩めない。
            .timeout(request_timeout)
            .build()
            .map_err(|error| IbkrError::Network(error.to_string()))?;

        Ok(Self {
            client,
            rate_limiter: IbkrRateLimiter::Shared(rate_limiter),
            max_wait,
        })
    }

    #[cfg(test)]
    pub(super) fn with_key_prefix(
        redis_url: &str,
        key_prefix: impl Into<String>,
        max_wait: Duration,
    ) -> Result<Self, IbkrError> {
        let rate_limiter = RateLimiter::new(redis_url, key_prefix).map_err(map_rate_limit_error)?;
        Self::with_rate_limiter(rate_limiter, max_wait, Duration::from_secs(10))
    }

    #[cfg(test)]
    pub(super) fn without_rate_limiter(
        max_wait: Duration,
        request_timeout: Duration,
    ) -> Result<Self, IbkrError> {
        let client = reqwest::Client::builder()
            .timeout(request_timeout)
            .build()
            .map_err(|error| IbkrError::Network(error.to_string()))?;

        Ok(Self {
            client,
            rate_limiter: IbkrRateLimiter::TestNoop,
            max_wait,
        })
    }

    pub(super) async fn send(
        &self,
        url: &Url,
        session_token: Option<&str>,
    ) -> Result<Response, IbkrError> {
        let quotas = quotas_for_url(url);
        let acquire_started = std::time::Instant::now();
        self.rate_limiter
            .acquire(&quotas, self.max_wait)
            .await
            .map_err(map_rate_limit_error)?;
        let acquire_duration = acquire_started.elapsed();
        if acquire_duration >= Duration::from_millis(100) {
            tracing::info!(
                wait_ms = acquire_duration.as_millis() as u64,
                "IBKR の共有 rate limit 取得に時間がかかりました"
            );
        }

        let mut request = self.client.get(url.clone());
        if let Some(token) = session_token {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .await
            .map_err(|error| IbkrError::Network(error.to_string()))?;

        if response.status().as_u16() == 429 {
            self.rate_limiter
                .penalize(RATE_LIMIT_GLOBAL_KEY, RATE_LIMIT_COOLDOWN)
                .await
                .map_err(map_rate_limit_error)?;
        }

        Ok(response)
    }
}

fn quotas_for_url(url: &Url) -> Vec<Quota> {
    let mut quotas = vec![Quota::new(
        RATE_LIMIT_GLOBAL_KEY,
        RATE_LIMIT_GLOBAL_PER_SECOND,
        RATE_LIMIT_PERIOD,
    )];

    if url.path().ends_with("/iserver/marketdata/history") {
        quotas.extend([
            Quota::new(
                RATE_LIMIT_HISTORY_SECOND_KEY,
                RATE_LIMIT_HISTORY_PER_SECOND,
                RATE_LIMIT_PERIOD,
            ),
            Quota::new(
                RATE_LIMIT_HISTORY_MINUTE_KEY,
                RATE_LIMIT_HISTORY_PER_MINUTE,
                Duration::from_secs(60),
            ),
        ]);
    }

    quotas
}

fn map_rate_limit_error(error: RateLimitError) -> IbkrError {
    match error {
        RateLimitError::MaxWaitExceeded { max_wait } => {
            IbkrError::RateLimitWaitExceeded { max_wait }
        }
        error => IbkrError::RateLimit(error.to_string()),
    }
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

    use reqwest::Url;
    use rstest::rstest;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[rstest]
    #[case::all_requests(
        "https://gateway.example.invalid/v1/api/trsrv/stocks?symbols=0000",
        vec![("ibkr:all".to_string(), 5, 1)],
    )]
    #[case::history_uses_all_three_quotas(
        "https://gateway.example.invalid/v1/api/iserver/marketdata/history?conid=1",
        vec![
            ("ibkr:all".to_string(), 5, 1),
            ("ibkr:history:second".to_string(), 5, 1),
            ("ibkr:history:minute".to_string(), 25, 60),
        ],
    )]
    fn selects_quotas_from_the_request_url(
        #[case] url: &str,
        #[case] expected: Vec<(String, u32, u64)>,
    ) -> Result<(), Box<dyn Error>> {
        let url = Url::parse(url)?;
        let quotas = quotas_for_url(&url)
            .into_iter()
            .map(|quota| (quota.key, quota.limit, quota.period.as_secs()))
            .collect::<Vec<_>>();

        assert_eq!(quotas, expected);
        Ok(())
    }

    #[tokio::test]
    async fn a_429_blocks_another_client_sharing_the_redis_prefix() -> Result<(), Box<dyn Error>> {
        let redis_url = std::env::var("REDIS_URL")?;
        let key_prefix = test_key_prefix();
        let max_wait = Duration::from_millis(50);
        let first =
            IbkrHttpClient::with_key_prefix(&redis_url, key_prefix.clone(), Duration::ZERO)?;
        let second = IbkrHttpClient::with_key_prefix(&redis_url, key_prefix, max_wait)?;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/endpoint", server.uri()))?;

        let first_status = first.send(&url, None).await?.status();
        let second_result = second.send(&url, None).await.map(|_| ());
        let received_requests = server
            .received_requests()
            .await
            .map(|requests| requests.len());

        assert_eq!(
            (first_status, second_result, received_requests),
            (
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                Err(IbkrError::RateLimitWaitExceeded { max_wait }),
                Some(1),
            ),
        );
        Ok(())
    }
}
