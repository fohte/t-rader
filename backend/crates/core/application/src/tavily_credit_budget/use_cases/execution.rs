use async_trait::async_trait;

use super::super::types::CachedTavilyOutcome;
use super::super::{
    ReservationClaim, SAFE_EXTRACT_RESERVATION, SAFE_SEARCH_RESERVATION,
    TavilyCreditBudgetOperation,
};
use super::helpers::{
    configuration_unavailable, confirmation_failed, idempotency_conflict, outcome_unknown,
    replay_extract, replay_search, valid_exact_usage, valid_usage,
};
use super::{
    TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError, TavilyCreditBudgetUseCasesPort,
};
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
        query: String,
        topic: Option<WebSearchTopic>,
        time_range: Option<WebSearchTimeRange>,
    ) -> Result<Vec<WebSearchResult>, TavilyCreditBudgetUseCasesError> {
        let operation = TavilyCreditBudgetOperation::Search;
        match self
            .prepare_reservation(&call_id, &request_fingerprint, operation)
            .await?
        {
            ReservationClaim::Replay(value) => {
                return replay_search(value);
            }
            ReservationClaim::Claimed => {}
            ReservationClaim::InProgress => return Err(outcome_unknown()),
            ReservationClaim::Conflict => return Err(idempotency_conflict()),
        }

        let provider = self
            .provider
            .as_ref()
            .ok_or_else(configuration_unavailable)?;
        let response = match provider.search(&query, topic, time_range).await {
            Ok(response) => response,
            Err(error) => {
                return Err(self
                    .provider_failure(&call_id, &request_fingerprint, operation, error)
                    .await);
            }
        };
        let Some(actual_credits) = response.credits_used else {
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        };
        if !valid_exact_usage(actual_credits, SAFE_SEARCH_RESERVATION) {
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        }

        self.settle(
            &call_id,
            &request_fingerprint,
            operation,
            actual_credits,
            CachedTavilyOutcome::SearchResults(response.results.clone()),
        )
        .await?;
        Ok(response.results)
    }

    async fn extract_page(
        &self,
        call_id: String,
        request_fingerprint: String,
        url: String,
    ) -> Result<String, TavilyCreditBudgetUseCasesError> {
        let operation = TavilyCreditBudgetOperation::Extract;
        match self
            .prepare_reservation(&call_id, &request_fingerprint, operation)
            .await?
        {
            ReservationClaim::Replay(value) => {
                return replay_extract(value);
            }
            ReservationClaim::Claimed => {}
            ReservationClaim::InProgress => return Err(outcome_unknown()),
            ReservationClaim::Conflict => return Err(idempotency_conflict()),
        }

        let provider = self
            .provider
            .as_ref()
            .ok_or_else(configuration_unavailable)?;
        let response: WebPageExtractionResponse = match provider.extract_page(&url).await {
            Ok(response) => response,
            Err(error) => {
                return Err(self
                    .provider_failure(&call_id, &request_fingerprint, operation, error)
                    .await);
            }
        };
        let Some(actual_credits) = response.credits_used else {
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        };
        if !valid_usage(actual_credits, SAFE_EXTRACT_RESERVATION) {
            let denial = confirmation_failed().denial();
            self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                .await;
            return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
        }

        match (response.raw_content, response.failure) {
            (Some(content), None) => {
                self.settle(
                    &call_id,
                    &request_fingerprint,
                    operation,
                    actual_credits,
                    CachedTavilyOutcome::ExtractedPage(content.clone()),
                )
                .await?;
                Ok(content)
            }
            (None, Some(failure)) => {
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
                let denial = confirmation_failed().denial();
                self.mark_unknown(&call_id, &request_fingerprint, operation, denial.clone())
                    .await;
                Err(TavilyCreditBudgetUseCasesError::Denied(denial))
            }
        }
    }
}
