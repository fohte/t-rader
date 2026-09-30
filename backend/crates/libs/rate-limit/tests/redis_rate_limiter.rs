use std::{error::Error, time::Duration};

use rate_limit::{Quota, RateLimitError, RateLimiter};
use rstest::{fixture, rstest};

fn test_limiter() -> Result<RateLimiter, Box<dyn Error>> {
    let redis_url = std::env::var("REDIS_URL")?;
    let prefix = format!("t-rader:test:rate-limit:{}:", std::process::id());
    Ok(RateLimiter::new(&redis_url, prefix)?)
}

#[tokio::test]
async fn acquire_consumes_all_quotas_atomically() -> Result<(), Box<dyn Error>> {
    let limiter = test_limiter()?;
    let fresh_quota = Quota::new("fresh", 1, Duration::from_secs(2));
    let occupied_quota = Quota::new("occupied", 1, Duration::from_secs(2));

    limiter
        .acquire(
            std::slice::from_ref(&occupied_quota),
            Duration::from_secs(1),
        )
        .await?;
    let combined_result = limiter
        .acquire(&[fresh_quota.clone(), occupied_quota], Duration::ZERO)
        .await;
    let fresh_result = limiter
        .acquire(std::slice::from_ref(&fresh_quota), Duration::from_secs(1))
        .await;

    assert_eq!(
        (combined_result, fresh_result),
        (
            Err(RateLimitError::MaxWaitExceeded {
                max_wait: Duration::ZERO,
            }),
            Ok(()),
        )
    );
    Ok(())
}

#[tokio::test]
async fn acquire_waits_for_quota_to_be_available() -> Result<(), Box<dyn Error>> {
    let limiter = test_limiter()?;
    let quota = Quota::new("paced", 1, Duration::from_millis(100));

    limiter
        .acquire(std::slice::from_ref(&quota), Duration::from_secs(1))
        .await?;
    let result = limiter
        .acquire(std::slice::from_ref(&quota), Duration::from_secs(1))
        .await;

    assert_eq!(result, Ok(()));
    Ok(())
}

#[tokio::test]
async fn penalize_blocks_matching_quotas_until_cooldown_expires() -> Result<(), Box<dyn Error>> {
    let limiter = test_limiter()?;
    let quota = Quota::new("shared", 1, Duration::from_secs(2));
    let cooldown = Duration::from_millis(100);

    limiter.penalize(&quota.key, cooldown).await?;
    let blocked_result = limiter
        .acquire(std::slice::from_ref(&quota), Duration::ZERO)
        .await;
    let resumed_result = limiter
        .acquire(std::slice::from_ref(&quota), Duration::from_secs(1))
        .await;

    assert_eq!(
        (blocked_result, resumed_result),
        (
            Err(RateLimitError::MaxWaitExceeded {
                max_wait: Duration::ZERO,
            }),
            Ok(()),
        )
    );
    Ok(())
}

#[tokio::test]
async fn acquire_enforces_fractional_quota_interval() -> Result<(), Box<dyn Error>> {
    let limiter = test_limiter()?;
    let quota = Quota::new("fractional", 3, Duration::from_secs(2));
    let first = limiter
        .acquire(std::slice::from_ref(&quota), Duration::ZERO)
        .await;
    let second = limiter
        .acquire(std::slice::from_ref(&quota), Duration::ZERO)
        .await;
    let third = limiter
        .acquire(std::slice::from_ref(&quota), Duration::ZERO)
        .await;
    let fourth = limiter
        .acquire(std::slice::from_ref(&quota), Duration::ZERO)
        .await;

    assert_eq!(
        (first, second, third, fourth),
        (
            Ok(()),
            Ok(()),
            Ok(()),
            Err(RateLimitError::MaxWaitExceeded {
                max_wait: Duration::ZERO,
            }),
        )
    );
    Ok(())
}

#[tokio::test]
async fn acquire_waits_until_max_wait_when_redis_is_unavailable() {
    let limiter = RateLimiter::new("redis://127.0.0.1:1/", "t-rader:test:unavailable:");
    let max_wait = Duration::from_millis(100);
    let started = tokio::time::Instant::now();
    let result = match limiter {
        Ok(limiter) => {
            limiter
                .acquire(
                    &[Quota::new("offline", 1, Duration::from_secs(1))],
                    max_wait,
                )
                .await
        }
        Err(error) => Err(error),
    };

    assert_eq!(
        (result, started.elapsed() >= max_wait),
        (Err(RateLimitError::MaxWaitExceeded { max_wait }), true,)
    );
}

#[fixture]
fn validation_limiter() -> Result<RateLimiter, RateLimitError> {
    RateLimiter::new("redis://127.0.0.1:1/", "t-rader:test:validation:")
}

#[rstest]
#[case::empty_key(vec![Quota::new("", 1, Duration::from_secs(1))], RateLimitError::EmptyKey)]
#[case::zero_limit(vec![Quota::new("limited", 0, Duration::from_secs(1))], RateLimitError::ZeroLimit)]
#[case::zero_period(vec![Quota::new("period", 1, Duration::ZERO)], RateLimitError::InvalidDuration)]
#[case::duplicate_key(vec![Quota::new("duplicate", 1, Duration::from_secs(1)), Quota::new("duplicate", 2, Duration::from_secs(1))], RateLimitError::DuplicateKey)]
#[tokio::test]
async fn acquire_rejects_invalid_quotas_without_connecting_to_redis(
    validation_limiter: Result<RateLimiter, RateLimitError>,
    #[case] quotas: Vec<Quota>,
    #[case] expected_error: RateLimitError,
) -> Result<(), Box<dyn Error>> {
    let limiter = validation_limiter?;
    assert_eq!(
        limiter.acquire(&quotas, Duration::ZERO).await,
        Err(expected_error)
    );
    Ok(())
}

#[test]
fn rate_limiter_rejects_empty_prefix() {
    assert_eq!(
        RateLimiter::new("redis://127.0.0.1:1/", "").map(|_| ()),
        Err(RateLimitError::EmptyPrefix)
    );
}

#[rstest]
#[tokio::test]
async fn penalize_rejects_empty_key(
    validation_limiter: Result<RateLimiter, RateLimitError>,
) -> Result<(), Box<dyn Error>> {
    let limiter = validation_limiter?;
    assert_eq!(
        limiter.penalize("", Duration::ZERO).await,
        Err(RateLimitError::EmptyKey)
    );
    Ok(())
}

#[rstest]
#[tokio::test]
async fn penalize_accepts_zero_duration(
    validation_limiter: Result<RateLimiter, RateLimitError>,
) -> Result<(), Box<dyn Error>> {
    let limiter = validation_limiter?;
    assert_eq!(limiter.penalize("valid", Duration::ZERO).await, Ok(()));
    Ok(())
}
