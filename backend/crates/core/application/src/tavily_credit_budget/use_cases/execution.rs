use async_trait::async_trait;
use tracing::{info, warn};

use super::super::types::CachedTavilyOutcome;
use super::super::{
    ReservationClaim, SAFE_EXTRACT_RESERVATION, SAFE_SEARCH_RESERVATION,
    TavilyCreditBudgetOperation, TavilyRetrieved,
};
use super::helpers::{
    configuration_unavailable, confirmation_failed, idempotency_conflict, outcome_unknown,
    provider_configuration_unavailable, replay_extract, replay_search, valid_exact_usage,
    valid_usage,
};
use super::retrieval_cache::{
    EXTRACT_CACHE_TTL, SEARCH_CACHE_TTL, cache_unavailable, release_cache_lease, store_cache_entry,
};
use super::{
    TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError, TavilyCreditBudgetUseCasesPort,
};
use crate::tavily_retrieval_cache::{TavilyRetrievalCacheKey, TavilyRetrievalCacheLookup};
use crate::web_search::{
    WebPageExtractionResponse, WebSearchError, WebSearchResult, WebSearchTimeRange, WebSearchTopic,
};

#[async_trait]
impl TavilyCreditBudgetUseCasesPort for TavilyCreditBudgetUseCases {
    async fn bootstrap(&self) -> Result<(), TavilyCreditBudgetUseCasesError> {
        TavilyCreditBudgetUseCases::bootstrap(self).await
    }

    async fn search(
        &self,
        call_id: String,
        request_fingerprint: String,
        authorization_scope: String,
        query: String,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<TavilyRetrieved<Vec<WebSearchResult>>, TavilyCreditBudgetUseCasesError> {
        let operation = TavilyCreditBudgetOperation::Search;
        if let Some(claim) = self
            .existing_claim(&call_id, &request_fingerprint, operation)
            .await?
        {
            return replay_search_claim(claim);
        }

        let provider = self
            .provider
            .as_ref()
            .ok_or_else(provider_configuration_unavailable)?;
        let config = self.config.as_ref().ok_or_else(configuration_unavailable)?;
        let credential_generation = self
            .credential_generation
            .ok_or_else(provider_configuration_unavailable)?;
        let cache_key = TavilyRetrievalCacheKey::search(
            &authorization_scope,
            config.account_id(),
            credential_generation,
            &query,
            topic,
            time_range,
        );
        let cache_lookup = self
            .retrieval_cache
            .acquire(cache_key, self.clock.now())
            .await
            .map_err(|_| cache_unavailable(operation))?;
        let cache_lease = match cache_lookup {
            TavilyRetrievalCacheLookup::Hit(entry) => {
                match serde_json::from_value::<Vec<WebSearchResult>>(entry.payload.clone()) {
                    Ok(results) => {
                        let outcome = CachedTavilyOutcome::SearchResultsWithFetchedAt {
                            results,
                            fetched_at: entry.fetched_at,
                        };
                        let recorded = match self
                            .record_cache_hit(
                                &call_id,
                                &request_fingerprint,
                                operation,
                                outcome,
                                self.clock.now(),
                            )
                            .await
                        {
                            Ok(recorded) => recorded,
                            Err(error) => {
                                return Err(error);
                            }
                        };
                        info!(
                            target: "tavily_retrieval_cache",
                            event = "cache_hit",
                            operation = operation.as_str(),
                            fetched_at = %entry.fetched_at.to_rfc3339(),
                            "Tavily retrieval cache hit"
                        );
                        return replay_search_claim(recorded);
                    }
                    Err(_) => {
                        warn!(
                            target: "tavily_retrieval_cache",
                            event = "invalid_entry",
                            operation = operation.as_str(),
                            "Tavily retrieval cache entry could not be decoded"
                        );
                        return Err(TavilyCreditBudgetUseCasesError::RetrievalCache);
                    }
                }
            }
            TavilyRetrievalCacheLookup::Miss(lease) => lease,
        };

        info!(
            target: "tavily_retrieval_cache",
            event = "cache_miss",
            operation = operation.as_str(),
            "Tavily retrieval cache miss"
        );
        let claim = match self
            .prepare_reservation(&call_id, &request_fingerprint, operation)
            .await
        {
            Ok(claim) => claim,
            Err(error) => {
                release_cache_lease(cache_lease, operation).await;
                return Err(error);
            }
        };
        match claim {
            ReservationClaim::Replay(value) => {
                release_cache_lease(cache_lease, operation).await;
                return replay_search(value);
            }
            ReservationClaim::Claimed => {}
            ReservationClaim::InProgress => {
                release_cache_lease(cache_lease, operation).await;
                return Err(outcome_unknown());
            }
            ReservationClaim::Conflict => {
                release_cache_lease(cache_lease, operation).await;
                return Err(idempotency_conflict());
            }
        }

        let response = match provider.search(&query, topic, time_range).await {
            Ok(response) => response,
            Err(error) => {
                release_cache_lease(cache_lease, operation).await;
                return Err(self
                    .provider_failure(&call_id, &request_fingerprint, operation, error)
                    .await);
            }
        };
        let Some(actual_credits) = response.credits_used else {
            release_cache_lease(cache_lease, operation).await;
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        };
        if !valid_exact_usage(actual_credits, SAFE_SEARCH_RESERVATION) {
            release_cache_lease(cache_lease, operation).await;
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        }

        let fetched_at = self.clock.now();
        self.settle(
            &call_id,
            &request_fingerprint,
            operation,
            actual_credits,
            CachedTavilyOutcome::SearchResultsWithFetchedAt {
                results: response.results.clone(),
                fetched_at,
            },
        )
        .await?;
        if let Ok(payload) = serde_json::to_value(&response.results) {
            store_cache_entry(
                cache_lease,
                payload,
                fetched_at,
                SEARCH_CACHE_TTL,
                operation,
            )
            .await;
        } else {
            release_cache_lease(cache_lease, operation).await;
        }
        Ok(TavilyRetrieved {
            value: response.results,
            fetched_at: Some(fetched_at),
        })
    }

    async fn extract_page(
        &self,
        call_id: String,
        request_fingerprint: String,
        authorization_scope: String,
        url: String,
    ) -> Result<TavilyRetrieved<String>, TavilyCreditBudgetUseCasesError> {
        let operation = TavilyCreditBudgetOperation::Extract;
        if let Some(claim) = self
            .existing_claim(&call_id, &request_fingerprint, operation)
            .await?
        {
            return replay_extract_claim(claim);
        }

        let provider = self
            .provider
            .as_ref()
            .ok_or_else(provider_configuration_unavailable)?;
        let config = self.config.as_ref().ok_or_else(configuration_unavailable)?;
        let credential_generation = self
            .credential_generation
            .ok_or_else(provider_configuration_unavailable)?;
        let cache_key = TavilyRetrievalCacheKey::extract(
            &authorization_scope,
            config.account_id(),
            credential_generation,
            &url,
        );
        let cache_lookup = self
            .retrieval_cache
            .acquire(cache_key, self.clock.now())
            .await
            .map_err(|_| cache_unavailable(operation))?;
        let cache_lease = match cache_lookup {
            TavilyRetrievalCacheLookup::Hit(entry) => {
                match serde_json::from_value::<String>(entry.payload.clone()) {
                    Ok(content) if !content.is_empty() => {
                        let outcome = CachedTavilyOutcome::ExtractedPageWithFetchedAt {
                            content,
                            fetched_at: entry.fetched_at,
                        };
                        let recorded = match self
                            .record_cache_hit(
                                &call_id,
                                &request_fingerprint,
                                operation,
                                outcome,
                                self.clock.now(),
                            )
                            .await
                        {
                            Ok(recorded) => recorded,
                            Err(error) => {
                                return Err(error);
                            }
                        };
                        info!(
                            target: "tavily_retrieval_cache",
                            event = "cache_hit",
                            operation = operation.as_str(),
                            fetched_at = %entry.fetched_at.to_rfc3339(),
                            "Tavily retrieval cache hit"
                        );
                        return replay_extract_claim(recorded);
                    }
                    _ => {
                        warn!(
                            target: "tavily_retrieval_cache",
                            event = "invalid_entry",
                            operation = operation.as_str(),
                            "Tavily retrieval cache entry could not be decoded"
                        );
                        return Err(TavilyCreditBudgetUseCasesError::RetrievalCache);
                    }
                }
            }
            TavilyRetrievalCacheLookup::Miss(lease) => lease,
        };

        info!(
            target: "tavily_retrieval_cache",
            event = "cache_miss",
            operation = operation.as_str(),
            "Tavily retrieval cache miss"
        );
        let claim = match self
            .prepare_reservation(&call_id, &request_fingerprint, operation)
            .await
        {
            Ok(claim) => claim,
            Err(error) => {
                release_cache_lease(cache_lease, operation).await;
                return Err(error);
            }
        };
        match claim {
            ReservationClaim::Replay(value) => {
                release_cache_lease(cache_lease, operation).await;
                return replay_extract(value);
            }
            ReservationClaim::Claimed => {}
            ReservationClaim::InProgress => {
                release_cache_lease(cache_lease, operation).await;
                return Err(outcome_unknown());
            }
            ReservationClaim::Conflict => {
                release_cache_lease(cache_lease, operation).await;
                return Err(idempotency_conflict());
            }
        }

        let response: WebPageExtractionResponse = match provider.extract_page(&url).await {
            Ok(response) => response,
            Err(error) => {
                release_cache_lease(cache_lease, operation).await;
                return Err(self
                    .provider_failure(&call_id, &request_fingerprint, operation, error)
                    .await);
            }
        };
        let Some(actual_credits) = response.credits_used else {
            release_cache_lease(cache_lease, operation).await;
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        };
        if !valid_usage(actual_credits, SAFE_EXTRACT_RESERVATION) {
            release_cache_lease(cache_lease, operation).await;
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        }

        match (response.raw_content, response.failure) {
            (Some(content), None) => {
                let fetched_at = self.clock.now();
                self.settle(
                    &call_id,
                    &request_fingerprint,
                    operation,
                    actual_credits,
                    CachedTavilyOutcome::ExtractedPageWithFetchedAt {
                        content: content.clone(),
                        fetched_at,
                    },
                )
                .await?;
                if content.is_empty() {
                    release_cache_lease(cache_lease, operation).await;
                } else if let Ok(payload) = serde_json::to_value(&content) {
                    store_cache_entry(
                        cache_lease,
                        payload,
                        fetched_at,
                        EXTRACT_CACHE_TTL,
                        operation,
                    )
                    .await;
                } else {
                    release_cache_lease(cache_lease, operation).await;
                }
                Ok(TavilyRetrieved {
                    value: content,
                    fetched_at: Some(fetched_at),
                })
            }
            (None, Some(failure)) => {
                release_cache_lease(cache_lease, operation).await;
                let error = WebSearchError::ExtractionFailed {
                    url: failure.url,
                    message: failure.message,
                };
                self.settle(
                    &call_id,
                    &request_fingerprint,
                    operation,
                    actual_credits,
                    CachedTavilyOutcome::ExtractionFailed(error.clone()),
                )
                .await?;
                Err(TavilyCreditBudgetUseCasesError::Provider(error))
            }
            _ => {
                release_cache_lease(cache_lease, operation).await;
                let denial = confirmation_failed().denial();
                self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                    .await;
                Err(TavilyCreditBudgetUseCasesError::Denied(denial))
            }
        }
    }
}

fn replay_search_claim(
    claim: ReservationClaim,
) -> Result<TavilyRetrieved<Vec<WebSearchResult>>, TavilyCreditBudgetUseCasesError> {
    match claim {
        ReservationClaim::Replay(value) => replay_search(value),
        ReservationClaim::InProgress => Err(outcome_unknown()),
        ReservationClaim::Conflict => Err(idempotency_conflict()),
        ReservationClaim::Claimed => Err(TavilyCreditBudgetUseCasesError::RetrievalCache),
    }
}

fn replay_extract_claim(
    claim: ReservationClaim,
) -> Result<TavilyRetrieved<String>, TavilyCreditBudgetUseCasesError> {
    match claim {
        ReservationClaim::Replay(value) => replay_extract(value),
        ReservationClaim::InProgress => Err(outcome_unknown()),
        ReservationClaim::Conflict => Err(idempotency_conflict()),
        ReservationClaim::Claimed => Err(TavilyCreditBudgetUseCasesError::RetrievalCache),
    }
}
