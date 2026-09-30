use std::{collections::HashSet, sync::Arc, time::Duration};

use redis::{Script, aio::ConnectionManager};
use thiserror::Error;
use tokio::{sync::Mutex, time::Instant};

const ACQUIRE_SCRIPT: &str = r#"
local now = redis.call('TIME')
local now_sec = tonumber(now[1])
local now_usec = tonumber(now[2])
local quota_count = #ARGV / 2
local retry_after_us = 0
local next_tats = {}

for index = 1, quota_count do
  local key_index = (index - 1) * 2
  local arg_index = (index - 1) * 2
  local quota_key = KEYS[key_index + 1]
  local penalty_key = KEYS[key_index + 2]
  local limit = tonumber(ARGV[arg_index + 1])
  local period_ms = tonumber(ARGV[arg_index + 2])
  local interval_us = math.ceil(period_ms * 1000 / limit)
  local burst_us = (limit - 1) * interval_us
  local tat_sec = now_sec
  local tat_usec = now_usec
  local stored_tat = redis.call('GET', quota_key)

  if stored_tat then
    local seconds, microseconds = string.match(stored_tat, '^(%d+):(%d+)$')
    tat_sec = tonumber(seconds)
    tat_usec = tonumber(microseconds)
  end

  local eligible_sec = tat_sec
  local eligible_usec = tat_usec - burst_us
  if eligible_usec < 0 then
    local borrowed_seconds = math.ceil(-eligible_usec / 1000000)
    eligible_sec = eligible_sec - borrowed_seconds
    eligible_usec = eligible_usec + borrowed_seconds * 1000000
  end
  local quota_retry_after_us =
    (eligible_sec - now_sec) * 1000000 + eligible_usec - now_usec
  local penalty = redis.call('GET', penalty_key)
  local penalty_ttl_ms = redis.call('PTTL', penalty_key)

  if quota_retry_after_us > retry_after_us then
    retry_after_us = quota_retry_after_us
  end
  if penalty and penalty_ttl_ms == 0 then
    penalty_ttl_ms = 1
  end
  local penalty_ttl_us = penalty_ttl_ms * 1000
  if penalty_ttl_us > retry_after_us then
    retry_after_us = penalty_ttl_us
  end

  if tat_sec < now_sec or (tat_sec == now_sec and tat_usec < now_usec) then
    tat_sec = now_sec
    tat_usec = now_usec
  end

  local next_usec = tat_usec + interval_us
  local next_sec = tat_sec + math.floor(next_usec / 1000000)
  next_usec = next_usec % 1000000
  next_tats[index] = string.format('%.0f:%06.0f', next_sec, next_usec)
end

if retry_after_us > 0 then
  return {0, math.ceil(retry_after_us / 1000)}
end

for index = 1, quota_count do
  local key_index = (index - 1) * 2
  local arg_index = (index - 1) * 2
  redis.call(
    'SET',
    KEYS[key_index + 1],
    tostring(next_tats[index]),
    'PX',
    ARGV[arg_index + 2]
  )
end

return {1, 0}
"#;

const PENALIZE_SCRIPT: &str = r#"
local current_ttl_ms = redis.call('PTTL', KEYS[1])
local requested_ttl_ms = tonumber(ARGV[1])

if current_ttl_ms < requested_ttl_ms then
  redis.call('SET', KEYS[1], '1', 'PX', requested_ttl_ms)
end

return 1
"#;

const INITIAL_RETRY_BACKOFF: Duration = Duration::from_secs(1);
const MAX_RETRY_BACKOFF: Duration = Duration::from_secs(30);
const REDIS_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_EXACT_DURATION_MILLIS: u64 = 8_000_000_000_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RateLimitError {
    #[error("rate limit acquisition exceeded the maximum wait of {max_wait:?}")]
    MaxWaitExceeded { max_wait: Duration },
    #[error("quota key must not be empty")]
    EmptyKey,
    #[error("Redis key prefix must not be empty")]
    EmptyPrefix,
    #[error("quota limit must be greater than zero")]
    ZeroLimit,
    #[error("duration exceeds the Redis Lua precision range")]
    InvalidDuration,
    #[error("quota keys must be unique within one acquire call")]
    DuplicateKey,
    #[error("Redis returned an invalid rate limit response")]
    InvalidRedisResponse,
    #[error("Redis operation failed: {0}")]
    Redis(String),
}

/// 1 つのキーに対する期間内の上限。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quota {
    pub key: String,
    pub limit: u32,
    pub period: Duration,
}

impl Quota {
    #[must_use]
    pub fn new(key: impl Into<String>, limit: u32, period: Duration) -> Self {
        Self {
            key: key.into(),
            limit,
            period,
        }
    }
}

/// Redis を使い、全プロセスで共有される rate limit を制御する。
#[derive(Clone)]
pub struct RateLimiter {
    client: redis::Client,
    connection_manager: Arc<Mutex<Option<ConnectionManager>>>,
    key_prefix: String,
}

impl RateLimiter {
    /// Redis URL とすべてのキーに付ける prefix から limiter を構築する。
    ///
    /// Redis への接続は初回操作まで遅延される。
    pub fn new(redis_url: &str, key_prefix: impl Into<String>) -> Result<Self, RateLimitError> {
        let key_prefix = key_prefix.into();
        if key_prefix.is_empty() {
            return Err(RateLimitError::EmptyPrefix);
        }

        let client = redis::Client::open(redis_url)
            .map_err(|error| RateLimitError::Redis(error.to_string()))?;

        Ok(Self {
            client,
            connection_manager: Arc::new(Mutex::new(None)),
            key_prefix,
        })
    }

    /// すべての quota を一度に取得する。枠が足りない場合は `max_wait` まで待機する。
    pub async fn acquire(
        &self,
        quotas: &[Quota],
        max_wait: Duration,
    ) -> Result<(), RateLimitError> {
        if quotas.is_empty() {
            return Ok(());
        }

        let quota_periods = validate_quotas(quotas)?;
        let keys = redis_keys(&self.key_prefix, quotas);
        let started = Instant::now();
        let mut backoff = INITIAL_RETRY_BACKOFF;

        loop {
            let remaining = max_wait.saturating_sub(started.elapsed());
            if remaining.is_zero() && !max_wait.is_zero() {
                return Err(RateLimitError::MaxWaitExceeded { max_wait });
            }
            let operation_timeout = if max_wait.is_zero() {
                REDIS_OPERATION_TIMEOUT
            } else {
                remaining.min(REDIS_OPERATION_TIMEOUT)
            };
            let call = self.run_acquire_script(quotas, &quota_periods, &keys);

            match tokio::time::timeout(operation_timeout, call).await {
                Ok(Ok((1, _))) => return Ok(()),
                Ok(Ok((0, retry_after_ms))) => {
                    let retry_after = Duration::from_millis(retry_after_ms);
                    if retry_after >= remaining {
                        return Err(RateLimitError::MaxWaitExceeded { max_wait });
                    }
                    tokio::time::sleep(retry_after).await;
                }
                Ok(Ok(_)) => return Err(RateLimitError::InvalidRedisResponse),
                Ok(Err(error)) if is_transient_redis_error(&error) => {
                    self.wait_before_retry(&error.to_string(), started, max_wait, &mut backoff)
                        .await?;
                }
                Ok(Err(error)) => return Err(RateLimitError::Redis(error.to_string())),
                Err(_) => {
                    self.wait_before_retry("request timed out", started, max_wait, &mut backoff)
                        .await?;
                }
            }
        }
    }

    /// `key` に cooldown を設定する。既存 cooldown より短い期間では上書きしない。
    /// Redis のエラーは再試行せず呼び出し元に返す。
    pub async fn penalize(&self, key: &str, duration: Duration) -> Result<(), RateLimitError> {
        if key.is_empty() {
            return Err(RateLimitError::EmptyKey);
        }
        if duration.is_zero() {
            return Ok(());
        }
        let duration_ms = duration_to_millis(duration)?;
        let redis_key = format!("{}penalty:{key}", self.key_prefix);
        let result = tokio::time::timeout(REDIS_OPERATION_TIMEOUT, async {
            let mut connection = self
                .connection_manager()
                .await
                .map_err(|error| RateLimitError::Redis(error.to_string()))?;
            Script::new(PENALIZE_SCRIPT)
                .key(redis_key)
                .arg(duration_ms)
                .invoke_async::<i32>(&mut connection)
                .await
                .map_err(|error| RateLimitError::Redis(error.to_string()))
        })
        .await
        .map_err(|_| RateLimitError::Redis("request timed out".to_owned()))??;

        if result == 1 {
            Ok(())
        } else {
            Err(RateLimitError::InvalidRedisResponse)
        }
    }

    async fn run_acquire_script(
        &self,
        quotas: &[Quota],
        quota_periods: &[u64],
        keys: &[(String, String)],
    ) -> redis::RedisResult<(i32, u64)> {
        let mut connection = self.connection_manager().await?;
        let script = Script::new(ACQUIRE_SCRIPT);
        let mut invocation = script.prepare_invoke();

        for (quota_key, penalty_key) in keys {
            invocation.key(quota_key).key(penalty_key);
        }
        for (quota, period_ms) in quotas.iter().zip(quota_periods) {
            invocation.arg(quota.limit).arg(period_ms);
        }

        invocation.invoke_async(&mut connection).await
    }

    async fn connection_manager(&self) -> redis::RedisResult<ConnectionManager> {
        let mut connection_manager = self.connection_manager.lock().await;
        if let Some(connection) = connection_manager.as_ref() {
            return Ok(connection.clone());
        }

        let config = redis::aio::ConnectionManagerConfig::new()
            .set_connection_timeout(REDIS_CONNECT_TIMEOUT)
            .set_response_timeout(REDIS_OPERATION_TIMEOUT);
        let connection = ConnectionManager::new_with_config(self.client.clone(), config).await?;
        *connection_manager = Some(connection.clone());
        Ok(connection)
    }

    async fn wait_before_retry(
        &self,
        error: &str,
        started: Instant,
        max_wait: Duration,
        backoff: &mut Duration,
    ) -> Result<(), RateLimitError> {
        let remaining = max_wait.saturating_sub(started.elapsed());
        tracing::warn!(
            error,
            backoff_ms = backoff.as_millis(),
            "Redis に接続できないため rate limit の取得を再試行します"
        );
        if remaining.is_zero() {
            return Err(RateLimitError::MaxWaitExceeded { max_wait });
        }

        let delay = (*backoff).min(remaining);
        tokio::time::sleep(delay).await;
        if delay == remaining {
            return Err(RateLimitError::MaxWaitExceeded { max_wait });
        }
        *backoff = backoff.saturating_mul(2).min(MAX_RETRY_BACKOFF);
        Ok(())
    }
}

fn validate_quotas(quotas: &[Quota]) -> Result<Vec<u64>, RateLimitError> {
    let mut keys = HashSet::with_capacity(quotas.len());
    let mut periods = Vec::with_capacity(quotas.len());

    for quota in quotas {
        if quota.key.is_empty() {
            return Err(RateLimitError::EmptyKey);
        }
        if quota.limit == 0 {
            return Err(RateLimitError::ZeroLimit);
        }
        if !keys.insert(quota.key.as_str()) {
            return Err(RateLimitError::DuplicateKey);
        }
        periods.push(duration_to_millis(quota.period)?);
    }

    Ok(periods)
}

fn duration_to_millis(duration: Duration) -> Result<u64, RateLimitError> {
    if duration.is_zero() {
        return Err(RateLimitError::InvalidDuration);
    }
    let milliseconds = duration.as_nanos().div_ceil(1_000_000);
    u64::try_from(milliseconds)
        .ok()
        .filter(|milliseconds| *milliseconds <= MAX_EXACT_DURATION_MILLIS)
        .map(|milliseconds| milliseconds.max(1))
        .ok_or(RateLimitError::InvalidDuration)
}

fn redis_keys(prefix: &str, quotas: &[Quota]) -> Vec<(String, String)> {
    quotas
        .iter()
        .map(|quota| {
            (
                format!("{prefix}quota:{}", quota.key),
                format!("{prefix}penalty:{}", quota.key),
            )
        })
        .collect()
}

fn is_transient_redis_error(error: &redis::RedisError) -> bool {
    error.is_connection_dropped() || error.is_io_error() || error.is_timeout()
}
