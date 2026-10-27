use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::info;

use super::super::{
    BudgetReadiness, ExistingReservation, ReservationClaim, ReservationCommand, ReservationOutcome,
    TavilyCreditBudgetConfig, TavilyCreditBudgetDenial, TavilyCreditBudgetOperation,
    TavilyCreditBudgetReason, TavilyCreditBudgetScope,
};
use super::helpers::{
    confirmation_failed, denied, log_denial, log_reconciliation, month_start, replay_claim,
    validate_account_usage,
};
use super::{TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError};

struct ReservationContext<'a> {
    call_id: &'a str,
    request_fingerprint: &'a str,
    operation: TavilyCreditBudgetOperation,
    monthly_provider_usage: Decimal,
    config: &'a TavilyCreditBudgetConfig,
    now: DateTime<Utc>,
}

impl TavilyCreditBudgetUseCases {
    pub(super) async fn prepare_reservation(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
    ) -> Result<ReservationClaim, TavilyCreditBudgetUseCasesError> {
        if let Some(claim) = self
            .existing_claim(call_id, request_fingerprint, operation)
            .await?
        {
            return Ok(claim);
        }

        let Some(config) = self.config.as_ref() else {
            return Err(self
                .persist_denial(
                    call_id,
                    request_fingerprint,
                    operation,
                    TavilyCreditBudgetDenial::new(
                        TavilyCreditBudgetReason::ConfigurationUnavailable,
                        TavilyCreditBudgetScope::Account,
                        None,
                    ),
                    self.clock.now(),
                )
                .await);
        };
        let Some(provider) = self.provider.as_ref() else {
            return Err(self
                .persist_denial(
                    call_id,
                    request_fingerprint,
                    operation,
                    TavilyCreditBudgetDenial::new(
                        TavilyCreditBudgetReason::ConfigurationUnavailable,
                        TavilyCreditBudgetScope::Provider,
                        None,
                    ),
                    self.clock.now(),
                )
                .await);
        };

        self.ensure_ready(call_id, request_fingerprint, operation, config)
            .await?;
        let now = self.clock.now();
        let monthly_provider_usage = self
            .confirmed_account_usage(
                provider.as_ref(),
                call_id,
                request_fingerprint,
                operation,
                now,
            )
            .await?;
        let reservation = self
            .reserve_credits(
                call_id,
                request_fingerprint,
                operation,
                config,
                monthly_provider_usage,
                now,
            )
            .await?;
        let context = ReservationContext {
            call_id,
            request_fingerprint,
            operation,
            monthly_provider_usage,
            config,
            now,
        };
        if let Some(claim) = self.resolve_reservation(&context, reservation).await? {
            return Ok(claim);
        }

        self.claim_reservation(call_id, request_fingerprint, operation)
            .await
    }

    pub(super) async fn existing_claim(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
    ) -> Result<Option<ReservationClaim>, TavilyCreditBudgetUseCasesError> {
        if call_id.trim().is_empty()
            || call_id.len() > 255
            || request_fingerprint.trim().is_empty()
            || request_fingerprint.len() != 64
        {
            return Err(denied(
                TavilyCreditBudgetReason::IdempotencyConflict,
                TavilyCreditBudgetScope::Account,
                None,
            ));
        }

        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| confirmation_failed())?;
        let existing = self
            .repository
            .find_reservation(&transaction, call_id, request_fingerprint)
            .await
            .map_err(|_| confirmation_failed())?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| confirmation_failed())?;
        match existing {
            ExistingReservation::Replay(outcome) => replay_claim(outcome, operation).map(Some),
            ExistingReservation::InProgress => Err(denied(
                TavilyCreditBudgetReason::OutcomeUnknown,
                TavilyCreditBudgetScope::Confirmation,
                None,
            )),
            ExistingReservation::Conflict => Err(denied(
                TavilyCreditBudgetReason::IdempotencyConflict,
                TavilyCreditBudgetScope::Account,
                None,
            )),
            ExistingReservation::Missing | ExistingReservation::Prepared => Ok(None),
        }
    }

    async fn ensure_ready(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        config: &TavilyCreditBudgetConfig,
    ) -> Result<(), TavilyCreditBudgetUseCasesError> {
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| confirmation_failed())?;
        let readiness = self
            .repository
            .readiness(&transaction, config)
            .await
            .map_err(|_| confirmation_failed())?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| confirmation_failed())?;
        let denial = match readiness {
            BudgetReadiness::Ready => return Ok(()),
            BudgetReadiness::NotInitialized => TavilyCreditBudgetDenial::new(
                TavilyCreditBudgetReason::BootstrapRequired,
                TavilyCreditBudgetScope::Account,
                None,
            ),
            BudgetReadiness::AccountMismatch => TavilyCreditBudgetDenial::new(
                TavilyCreditBudgetReason::AccountMismatch,
                TavilyCreditBudgetScope::Account,
                None,
            ),
            BudgetReadiness::ConfigurationChanged => TavilyCreditBudgetDenial::new(
                TavilyCreditBudgetReason::ConfigurationChanged,
                TavilyCreditBudgetScope::Account,
                None,
            ),
        };
        Err(self
            .persist_denial(
                call_id,
                request_fingerprint,
                operation,
                denial,
                self.clock.now(),
            )
            .await)
    }

    async fn confirmed_account_usage(
        &self,
        provider: &dyn crate::web_search::WebSearchClient,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        now: DateTime<Utc>,
    ) -> Result<rust_decimal::Decimal, TavilyCreditBudgetUseCasesError> {
        let usage = match provider.account_usage().await {
            Ok(usage) => usage,
            Err(error) => {
                tracing::warn!(
                    target: "tavily_credit_budget",
                    event = "usage_confirmation_failed",
                    error_kind = error.kind(),
                    "Tavily account usage could not be confirmed"
                );
                let denial = confirmation_failed().denial();
                return Err(self
                    .persist_denial(call_id, request_fingerprint, operation, denial, now)
                    .await);
            }
        };
        if let Err(error) = validate_account_usage(
            &usage.current_plan,
            usage.plan_credits_used,
            usage.paygo_credits_used,
        ) {
            return Err(self
                .persist_denial(call_id, request_fingerprint, operation, error.denial(), now)
                .await);
        }
        let Some(monthly_provider_usage) = usage.total_credits_used() else {
            return Err(self
                .persist_denial(
                    call_id,
                    request_fingerprint,
                    operation,
                    confirmation_failed().denial(),
                    now,
                )
                .await);
        };
        Ok(monthly_provider_usage)
    }

    async fn reserve_credits(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
        config: &TavilyCreditBudgetConfig,
        monthly_provider_usage: Decimal,
        now: DateTime<Utc>,
    ) -> Result<ReservationOutcome, TavilyCreditBudgetUseCasesError> {
        let current_day = now.date_naive();
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| confirmation_failed())?;
        let reservation = self
            .repository
            .reserve(
                &transaction,
                ReservationCommand {
                    config: config.clone(),
                    call_id: call_id.to_owned(),
                    request_fingerprint: request_fingerprint.to_owned(),
                    operation,
                    monthly_provider_usage,
                    month_start: month_start(current_day).ok_or_else(confirmation_failed)?,
                    day_start: current_day,
                    now,
                },
            )
            .await
            .map_err(|_| confirmation_failed())?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| confirmation_failed())?;
        Ok(reservation)
    }

    async fn resolve_reservation(
        &self,
        context: &ReservationContext<'_>,
        reservation: ReservationOutcome,
    ) -> Result<Option<ReservationClaim>, TavilyCreditBudgetUseCasesError> {
        match reservation {
            ReservationOutcome::Reserved {
                daily_used,
                monthly_used,
            }
            | ReservationOutcome::ReusedPrepared {
                daily_used,
                monthly_used,
            } => {
                log_reconciliation(
                    context.monthly_provider_usage,
                    daily_used,
                    monthly_used,
                    context.config,
                );
                info!(
                    target: "tavily_credit_budget",
                    event = "reservation_created",
                    operation = context.operation.as_str(),
                    reserved_credits = %context.operation.reservation(),
                    daily_used = %daily_used,
                    daily_limit = %context.config.daily_limit(),
                    monthly_used = %monthly_used,
                    monthly_limit = %context.config.monthly_limit(),
                    "Tavily account credits reserved"
                );
                Ok(None)
            }
            ReservationOutcome::Replay(outcome) => {
                replay_claim(outcome, context.operation).map(Some)
            }
            ReservationOutcome::InProgress => Err(denied(
                TavilyCreditBudgetReason::OutcomeUnknown,
                TavilyCreditBudgetScope::Confirmation,
                None,
            )),
            ReservationOutcome::Denied {
                denial,
                daily_used,
                monthly_used,
            } => {
                log_reconciliation(
                    context.monthly_provider_usage,
                    daily_used,
                    monthly_used,
                    context.config,
                );
                log_denial(
                    &denial,
                    context.operation,
                    Some(daily_used),
                    Some(monthly_used),
                );
                self.record_denial(
                    context.call_id,
                    context.request_fingerprint,
                    context.operation,
                    denial.clone(),
                    context.now,
                )
                .await;
                Err(TavilyCreditBudgetUseCasesError::Denied(denial))
            }
            ReservationOutcome::Conflict => Err(denied(
                TavilyCreditBudgetReason::IdempotencyConflict,
                TavilyCreditBudgetScope::Account,
                None,
            )),
        }
    }

    async fn claim_reservation(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
    ) -> Result<ReservationClaim, TavilyCreditBudgetUseCasesError> {
        let transaction = self
            .unit_of_work
            .begin()
            .await
            .map_err(|_| confirmation_failed())?;
        let claim = self
            .repository
            .claim(&transaction, call_id, request_fingerprint, self.clock.now())
            .await
            .map_err(|_| confirmation_failed())?;
        self.unit_of_work
            .commit(transaction)
            .await
            .map_err(|_| confirmation_failed())?;
        match claim {
            ReservationClaim::Claimed => Ok(ReservationClaim::Claimed),
            ReservationClaim::InProgress => Err(denied(
                TavilyCreditBudgetReason::OutcomeUnknown,
                TavilyCreditBudgetScope::Confirmation,
                None,
            )),
            ReservationClaim::Conflict => Err(denied(
                TavilyCreditBudgetReason::IdempotencyConflict,
                TavilyCreditBudgetScope::Account,
                None,
            )),
            ReservationClaim::Replay(outcome) => replay_claim(outcome, operation),
        }
    }
}
