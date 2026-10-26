use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use tracing::warn;

use super::super::types::CachedTavilyOutcome;
use super::super::{RecordCacheHitCommand, ReservationClaim, TavilyCreditBudgetOperation};
use super::{TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError};
use crate::tavily_retrieval_cache::{TavilyRetrievalCacheLease, TavilyRetrievalCacheWrite};

pub(super) const SEARCH_CACHE_TTL: Duration = Duration::minutes(15);
pub(super) const EXTRACT_CACHE_TTL: Duration = Duration::hours(24);

impl TavilyCreditBudgetUseCases {
    pub(super) async fn record_cache_hit(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        outcome: CachedTavilyOutcome,
        now: DateTime<Utc>,
    ) -> Result<ReservationClaim, TavilyCreditBudgetUseCasesError> {
        let outcome = serde_json::to_value(outcome)
            .map_err(|_| TavilyCreditBudgetUseCasesError::RetrievalCache)?;
        let transaction = self.unit_of_work.begin().await.map_err(|error| {
            warn!(
                target: "tavily_retrieval_cache",
                event = "cache_hit_record_failed",
                operation = operation.as_str(),
                phase = "begin",
                error = %error,
                "Tavily retrieval cache hit could not be recorded"
            );
            TavilyCreditBudgetUseCasesError::RetrievalCache
        })?;
        let claim = self
            .repository
            .record_cache_hit(
                &transaction,
                RecordCacheHitCommand {
                    call_id: call_id.to_owned(),
                    request_fingerprint: request_fingerprint.to_owned(),
                    operation,
                    outcome,
                    now,
                },
            )
            .await
            .map_err(|error| {
                warn!(
                    target: "tavily_retrieval_cache",
                    event = "cache_hit_record_failed",
                    operation = operation.as_str(),
                    phase = "repository",
                    error = %error,
                    "Tavily retrieval cache hit could not be recorded"
                );
                TavilyCreditBudgetUseCasesError::RetrievalCache
            })?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|error| {
                warn!(
                    target: "tavily_retrieval_cache",
                    event = "cache_hit_record_failed",
                    operation = operation.as_str(),
                    phase = "commit",
                    error = %error,
                    "Tavily retrieval cache hit could not be recorded"
                );
                TavilyCreditBudgetUseCasesError::RetrievalCache
            })?;
        Ok(claim)
    }
}

pub(super) async fn store_cache_entry(
    lease: Box<dyn TavilyRetrievalCacheLease>,
    payload: Value,
    fetched_at: DateTime<Utc>,
    ttl: Duration,
    operation: TavilyCreditBudgetOperation,
) {
    let Some(expires_at) = fetched_at.checked_add_signed(ttl) else {
        release_cache_lease(lease, operation).await;
        return;
    };
    if lease
        .store(TavilyRetrievalCacheWrite {
            payload,
            fetched_at,
            expires_at,
        })
        .await
        .is_err()
    {
        warn!(
            target: "tavily_retrieval_cache",
            event = "cache_store_failed",
            operation = operation.as_str(),
            "Tavily retrieval cache entry could not be stored"
        );
    }
}

pub(super) async fn release_cache_lease(
    lease: Box<dyn TavilyRetrievalCacheLease>,
    operation: TavilyCreditBudgetOperation,
) {
    if lease.release().await.is_err() {
        warn!(
            target: "tavily_retrieval_cache",
            event = "cache_lock_release_failed",
            operation = operation.as_str(),
            "Tavily retrieval cache lock could not be released cleanly"
        );
    }
}

pub(super) fn cache_unavailable(
    operation: TavilyCreditBudgetOperation,
) -> TavilyCreditBudgetUseCasesError {
    warn!(
        target: "tavily_retrieval_cache",
        event = "cache_lock_failed",
        operation = operation.as_str(),
        "Tavily retrieval cache lock could not be acquired"
    );
    TavilyCreditBudgetUseCasesError::RetrievalCache
}
