use std::{
    error::Error,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rate_limit::{Quota, RateLimitError, RateLimiter};

static NEXT_PREFIX: AtomicU64 = AtomicU64::new(0);

fn test_limiter() -> Result<RateLimiter, Box<dyn Error>> {
    let redis_url = std::env::var("REDIS_URL")?;
    let prefix = format!(
        "t-rader:test:rate-limit:{}:{}:",
        std::process::id(),
        NEXT_PREFIX.fetch_add(1, Ordering::Relaxed)
    );
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
