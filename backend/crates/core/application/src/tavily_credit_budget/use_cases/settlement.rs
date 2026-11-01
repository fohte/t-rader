use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use tracing::{info, warn};

use super::super::types::CachedTavilyOutcome;
use super::super::{
    FinishFailureCommand, RecordDenialCommand, ReservationFailureState, TavilyCreditBudgetDenial,
    TavilyCreditBudgetOperation, TavilyCreditBudgetReason, TavilyCreditBudgetScope,
};
use super::helpers::{confirmation_failed, log_denial};
use super::{TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError};
use crate::web_search::WebSearchError;

impl TavilyCreditBudgetUseCases {
    pub(super) async fn record_denial(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        denial: TavilyCreditBudgetDenial,
        now: DateTime<Utc>,
    ) {
        let outcome = match serde_json::to_value(CachedTavilyOutcome::Denied(denial.clone())) {
            Ok(outcome) => outcome,
            Err(_) => return,
        };
        let Ok(transaction) = self.unit_of_work.begin().await else {
            warn!(
                target: "tavily_credit_budget",
                event = "denial_record_failed",
                operation = operation.as_str(),
                "Tavily credit denial could not be persisted"
            );
            return;
        };
        if self
            .repository
            .record_denial(
                &transaction,
                RecordDenialCommand {
                    call_id: call_id.to_owned(),
                    request_fingerprint: request_fingerprint.to_owned(),
                    operation,
                    outcome,
                    now,
                },
            )
            .await
            .is_err()
            || self.unit_of_work.commit(transaction).await.is_err()
        {
            warn!(
                target: "tavily_credit_budget",
                event = "denial_record_failed",
                operation = operation.as_str(),
                "Tavily credit denial could not be persisted"
            );
        }
    }

    pub(super) async fn persist_denial(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        denial: TavilyCreditBudgetDenial,
        now: DateTime<Utc>,
    ) -> TavilyCreditBudgetUseCasesError {
        self.record_denial(call_id, request_fingerprint, operation, denial.clone(), now)
            .await;
        log_denial(&denial, operation, None, None);
        TavilyCreditBudgetUseCasesError::Denied(denial)
    }

    pub(super) async fn mark_unknown(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        denial: TavilyCreditBudgetDenial,
    ) {
        let outcome = match serde_json::to_value(CachedTavilyOutcome::Denied(denial.clone())) {
            Ok(outcome) => outcome,
            Err(_) => return,
        };
        let Ok(transaction) = self.unit_of_work.begin().await else {
            warn!(
                target: "tavily_credit_budget",
                event = "unknown_outcome_persist_failed",
                operation = operation.as_str(),
                "Tavily unknown outcome could not be persisted"
            );
            return;
        };
        if self
            .repository
            .finish_failure(
                &transaction,
                FinishFailureCommand {
                    call_id: call_id.to_owned(),
                    request_fingerprint: request_fingerprint.to_owned(),
                    state: ReservationFailureState::Unknown,
                    outcome,
                    now: self.clock.now(),
                },
            )
            .await
            .is_err()
            || self.unit_of_work.commit(transaction).await.is_err()
        {
            warn!(
                target: "tavily_credit_budget",
                event = "unknown_outcome_persist_failed",
                operation = operation.as_str(),
                "Tavily unknown outcome could not be persisted"
            );
        }
    }

    pub(super) async fn settle(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        actual_credits: Decimal,
        outcome: CachedTavilyOutcome,
    ) -> Result<(), TavilyCreditBudgetUseCasesError> {
        let serialized = serde_json::to_value(outcome).map_err(|_| confirmation_failed())?;
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| confirmation_failed())?;
        self.repository
            .settle(
                &transaction,
                call_id,
                request_fingerprint,
                actual_credits,
                serialized,
                self.clock.now(),
            )
            .await
            .map_err(|_| confirmation_failed())?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| confirmation_failed())?;
        info!(
            target: "tavily_credit_budget",
            event = "reservation_settled",
            operation = operation.as_str(),
            reserved_credits = %operation.reservation(),
            settled_credits = %actual_credits,
            "Tavily account credits settled"
        );
        Ok(())
    }

    pub(super) async fn provider_failure(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        error: WebSearchError,
    ) -> TavilyCreditBudgetUseCasesError {
        warn!(
            target: "tavily_credit_budget",
            event = "provider_call_failed",
            operation = operation.as_str(),
            error_kind = error.kind(),
            "Tavily provider call failed"
        );
        let (state, denial) = match error {
            WebSearchError::Api { status, .. } => (
                ReservationFailureState::ProviderRejected,
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::ProviderRejected,
                    TavilyCreditBudgetScope::Provider,
                    None,
                )
                .with_provider_status(status),
            ),
            WebSearchError::Usage(_) | WebSearchError::UsageTimeout => (
                ReservationFailureState::Unknown,
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::UsageConfirmationFailed,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            ),
            _ => (
                ReservationFailureState::Unknown,
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            ),
        };
        let outcome = match serde_json::to_value(CachedTavilyOutcome::Denied(denial.clone())) {
            Ok(outcome) => outcome,
            Err(_) => Value::Null,
        };
        let transaction = self.unit_of_work.begin().await;
        let persisted = if let Ok(transaction) = transaction {
            let persisted = self
                .repository
                .finish_failure(
                    &transaction,
                    FinishFailureCommand {
                        call_id: call_id.to_owned(),
                        request_fingerprint: request_fingerprint.to_owned(),
                        state,
                        outcome,
                        now: self.clock.now(),
                    },
                )
                .await;
            match persisted {
                Ok(()) => self.unit_of_work.commit(transaction).await.is_ok(),
                Err(_) => false,
            }
        } else {
            false
        };
        if !persisted {
            warn!(
                target: "tavily_credit_budget",
                event = "provider_failure_persist_failed",
                operation = operation.as_str(),
                "Tavily provider failure could not be persisted"
            );
        }
        log_denial(&denial, operation, None, None);
        TavilyCreditBudgetUseCasesError::Denied(denial)
    }
}
