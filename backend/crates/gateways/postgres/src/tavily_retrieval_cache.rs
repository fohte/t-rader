use std::time::{Duration as StdDuration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Duration, FixedOffset, Utc};
use core_application::{
    tavily_retrieval_cache::{
        TavilyRetrievalCacheEntry, TavilyRetrievalCacheError, TavilyRetrievalCacheKey,
        TavilyRetrievalCacheLease, TavilyRetrievalCacheLookup, TavilyRetrievalCacheRepository,
        TavilyRetrievalCacheWrite,
    },
    unit_of_work::SharedUnitOfWork,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, Statement};
use tracing::warn;
use uuid::Uuid;

use crate::transaction::transaction_ref;

#[cfg(test)]
#[path = "tavily_retrieval_cache/tests.rs"]
mod tests;

const EXPIRED_CACHE_CLEANUP_LIMIT: i64 = 100;
const CACHE_FILL_LEASE_TTL: Duration = Duration::minutes(5);
const CACHE_LOOKUP_POLL_INTERVAL: StdDuration = StdDuration::from_millis(50);
const CACHE_LOOKUP_WAIT_LIMIT: StdDuration = StdDuration::from_secs(90);

#[derive(Clone)]
pub struct PostgresTavilyRetrievalCacheRepository {
    unit_of_work: SharedUnitOfWork,
}

impl PostgresTavilyRetrievalCacheRepository {
    pub fn new(unit_of_work: SharedUnitOfWork) -> Self {
        Self { unit_of_work }
    }
}

#[async_trait]
impl TavilyRetrievalCacheRepository for PostgresTavilyRetrievalCacheRepository {
    async fn acquire(
        &self,
        key: TavilyRetrievalCacheKey,
        now: DateTime<Utc>,
    ) -> Result<TavilyRetrievalCacheLookup, TavilyRetrievalCacheError> {
        let started_at = Instant::now();
        loop {
            let elapsed_std = started_at.elapsed();
            if elapsed_std >= CACHE_LOOKUP_WAIT_LIMIT {
                return Err(TavilyRetrievalCacheError::Storage);
            }
            let elapsed =
                Duration::from_std(elapsed_std).map_err(|_| TavilyRetrievalCacheError::Storage)?;
            let lookup_time = now
                .checked_add_signed(elapsed)
                .ok_or(TavilyRetrievalCacheError::Storage)?;
            if let Some(lookup) = acquire_once(&self.unit_of_work, &key, lookup_time).await? {
                return Ok(lookup);
            }
            tokio::time::sleep(
                CACHE_LOOKUP_POLL_INTERVAL.min(CACHE_LOOKUP_WAIT_LIMIT - elapsed_std),
            )
            .await;
        }
    }
}

async fn acquire_once(
    unit_of_work: &SharedUnitOfWork,
    key: &TavilyRetrievalCacheKey,
    now: DateTime<Utc>,
) -> Result<Option<TavilyRetrievalCacheLookup>, TavilyRetrievalCacheError> {
    let transaction = unit_of_work
        .begin()
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;
    let database_transaction =
        transaction_ref(&transaction).ok_or(TavilyRetrievalCacheError::InvalidTransaction)?;
    lock_key(database_transaction, key).await?;
    let key_bytes = key.as_bytes().to_vec();
    database_transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM tavily_retrieval_cache WHERE cache_key = $1 AND \
             ((payload_json IS NOT NULL AND expires_at <= $2) OR \
              (lease_token IS NOT NULL AND lease_expires_at <= $2))",
            [key_bytes.clone().into(), now.fixed_offset().into()],
        ))
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;
    database_transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM tavily_retrieval_cache WHERE cache_key IN ( \
             SELECT cache_key FROM tavily_retrieval_cache \
             WHERE expires_at <= $1 OR lease_expires_at <= $1 \
             ORDER BY COALESCE(expires_at, lease_expires_at) \
             LIMIT $2 FOR UPDATE SKIP LOCKED)",
            [
                now.fixed_offset().into(),
                EXPIRED_CACHE_CLEANUP_LIMIT.into(),
            ],
        ))
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;

    let row = database_transaction
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT kind, payload_json, fetched_at, lease_token, lease_expires_at \
             FROM tavily_retrieval_cache WHERE cache_key = $1",
            [key_bytes.clone().into()],
        ))
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;

    if let Some(row) = row {
        let kind: String = row
            .try_get("", "kind")
            .map_err(|_| TavilyRetrievalCacheError::Storage)?;
        if kind != key.kind().as_str() {
            return Err(TavilyRetrievalCacheError::Storage);
        }
        let payload: Option<serde_json::Value> = row
            .try_get("", "payload_json")
            .map_err(|_| TavilyRetrievalCacheError::Storage)?;
        match payload {
            Some(payload) => {
                let fetched_at: DateTime<FixedOffset> = row
                    .try_get("", "fetched_at")
                    .map_err(|_| TavilyRetrievalCacheError::Storage)?;
                unit_of_work
                    .commit(transaction)
                    .await
                    .map_err(|_| TavilyRetrievalCacheError::Storage)?;
                return Ok(Some(TavilyRetrievalCacheLookup::Hit(
                    TavilyRetrievalCacheEntry {
                        payload,
                        fetched_at: fetched_at.with_timezone(&Utc),
                    },
                )));
            }
            None => {
                let lease_token: Option<Uuid> = row
                    .try_get("", "lease_token")
                    .map_err(|_| TavilyRetrievalCacheError::Storage)?;
                let lease_expires_at: DateTime<FixedOffset> =
                    row.try_get("", "lease_expires_at")
                        .map_err(|_| TavilyRetrievalCacheError::Storage)?;
                if lease_token.is_none() || lease_expires_at <= now.fixed_offset() {
                    return Err(TavilyRetrievalCacheError::Storage);
                }
                unit_of_work
                    .commit(transaction)
                    .await
                    .map_err(|_| TavilyRetrievalCacheError::Storage)?;
                return Ok(None);
            }
        }
    }

    let lease_token = Uuid::new_v4();
    let lease_expires_at = now
        .checked_add_signed(CACHE_FILL_LEASE_TTL)
        .ok_or(TavilyRetrievalCacheError::Storage)?;
    database_transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO tavily_retrieval_cache \
             (cache_key, kind, lease_token, lease_expires_at) VALUES ($1, $2, $3, $4)",
            [
                key_bytes.clone().into(),
                key.kind().as_str().into(),
                lease_token.into(),
                lease_expires_at.fixed_offset().into(),
            ],
        ))
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;
    unit_of_work
        .commit(transaction)
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;
    Ok(Some(TavilyRetrievalCacheLookup::Miss(Box::new(
        PostgresTavilyRetrievalCacheLease {
            unit_of_work: unit_of_work.clone(),
            key_bytes,
            lease_token,
        },
    ))))
}

async fn lock_key(
    transaction: &DatabaseTransaction,
    key: &TavilyRetrievalCacheKey,
) -> Result<(), TavilyRetrievalCacheError> {
    let digest = key.as_bytes();
    let lock_id = i64::from_be_bytes(
        digest[..8]
            .try_into()
            .map_err(|_| TavilyRetrievalCacheError::Storage)?,
    );
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock($1)",
            [lock_id.into()],
        ))
        .await
        .map_err(|_| TavilyRetrievalCacheError::Storage)?;
    Ok(())
}

struct PostgresTavilyRetrievalCacheLease {
    unit_of_work: SharedUnitOfWork,
    key_bytes: Vec<u8>,
    lease_token: Uuid,
}

impl PostgresTavilyRetrievalCacheLease {
    async fn store_entry(
        &self,
        entry: TavilyRetrievalCacheWrite,
    ) -> Result<(), TavilyRetrievalCacheError> {
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| TavilyRetrievalCacheError::Storage)?;
        let database_transaction =
            transaction_ref(&transaction).ok_or(TavilyRetrievalCacheError::InvalidTransaction)?;
        let result = database_transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE tavily_retrieval_cache SET payload_json = $1, fetched_at = $2, expires_at = $3, lease_token = NULL, lease_expires_at = NULL \
                 WHERE cache_key = $4 AND lease_token = $5 AND payload_json IS NULL",
                [
                    entry.payload.into(),
                    entry.fetched_at.fixed_offset().into(),
                    entry.expires_at.fixed_offset().into(),
                    self.key_bytes.clone().into(),
                    self.lease_token.into(),
                ],
            ))
            .await
            .map_err(|_| TavilyRetrievalCacheError::Storage)?;
        if result.rows_affected() != 1 {
            return Err(TavilyRetrievalCacheError::Storage);
        }
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| TavilyRetrievalCacheError::Storage)
    }

    async fn release_claim(&self) -> Result<(), TavilyRetrievalCacheError> {
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| TavilyRetrievalCacheError::Storage)?;
        let database_transaction =
            transaction_ref(&transaction).ok_or(TavilyRetrievalCacheError::InvalidTransaction)?;
        database_transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "DELETE FROM tavily_retrieval_cache WHERE cache_key = $1 AND lease_token = $2 AND payload_json IS NULL",
                [self.key_bytes.clone().into(), self.lease_token.into()],
            ))
            .await
            .map_err(|_| TavilyRetrievalCacheError::Storage)?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| TavilyRetrievalCacheError::Storage)
    }
}

#[async_trait]
impl TavilyRetrievalCacheLease for PostgresTavilyRetrievalCacheLease {
    async fn store(
        self: Box<Self>,
        entry: TavilyRetrievalCacheWrite,
    ) -> Result<(), TavilyRetrievalCacheError> {
        let result = self.store_entry(entry).await;
        if result.is_err() && self.release_claim().await.is_err() {
            warn!(
                target: "tavily_retrieval_cache",
                event = "cache_claim_cleanup_failed",
                "Tavily retrieval cache claim could not be released after a failed store"
            );
        }
        result
    }

    async fn release(self: Box<Self>) -> Result<(), TavilyRetrievalCacheError> {
        self.release_claim().await
    }
}
