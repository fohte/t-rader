use tracing::info;

use super::super::{
    BudgetReadiness, ExistingReservation, ReservationClaim, ReservationCommand, ReservationOutcome,
    TavilyCreditBudgetDenial, TavilyCreditBudgetOperation, TavilyCreditBudgetReason,
    TavilyCreditBudgetScope,
};
use super::helpers::{
    confirmation_failed, denied, log_denial, log_reconciliation, month_start, replay_claim,
    validate_account_usage,
};
use super::{TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError};
impl TavilyCreditBudgetUseCases {
    pub(super) async fn prepare_reservation(
        &self,
        call_id: &str,
        request_fingerprint: &str,
        operation: TavilyCreditBudgetOperation,
    ) -> Result<ReservationClaim, TavilyCreditBudgetUseCasesError> {
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
            ExistingReservation::Replay(outcome) => {
                return replay_claim(outcome, operation);
            }
            ExistingReservation::InProgress => {
                return Err(denied(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ));
            }
            ExistingReservation::Conflict => {
                return Err(denied(
                    TavilyCreditBudgetReason::IdempotencyConflict,
                    TavilyCreditBudgetScope::Account,
                    None,
                ));
            }
            ExistingReservation::Missing | ExistingReservation::Prepared => {}
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
        match readiness {
            BudgetReadiness::Ready => {}
            BudgetReadiness::NotInitialized => {
                return Err(self
                    .persist_denial(
                        call_id,
                        request_fingerprint,
                        operation,
                        TavilyCreditBudgetDenial::new(
                            TavilyCreditBudgetReason::BootstrapRequired,
                            TavilyCreditBudgetScope::Account,
                            None,
                        ),
                        self.clock.now(),
                    )
                    .await);
            }
            BudgetReadiness::AccountMismatch => {
                return Err(self
                    .persist_denial(
                        call_id,
                        request_fingerprint,
                        operation,
                        TavilyCreditBudgetDenial::new(
                            TavilyCreditBudgetReason::AccountMismatch,
                            TavilyCreditBudgetScope::Account,
                            None,
                        ),
                        self.clock.now(),
                    )
                    .await);
            }
            BudgetReadiness::ConfigurationChanged => {
                return Err(self
                    .persist_denial(
                        call_id,
                        request_fingerprint,
                        operation,
                        TavilyCreditBudgetDenial::new(
                            TavilyCreditBudgetReason::ConfigurationChanged,
                            TavilyCreditBudgetScope::Account,
                            None,
                        ),
                        self.clock.now(),
                    )
                    .await);
            }
        }

        let now = self.clock.now();
        let usage = match provider.account_usage().await {
            Ok(usage) => usage,
            Err(_) => {
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

        match reservation {
            ReservationOutcome::Reserved {
                daily_used,
                monthly_used,
            }
            | ReservationOutcome::ReusedPrepared {
                daily_used,
                monthly_used,
            } => {
                log_reconciliation(monthly_provider_usage, daily_used, monthly_used, config);
                info!(
                    target: "tavily_credit_budget",
                    event = "reservation_created",
                    operation = operation.as_str(),
                    reserved_credits = %operation.reservation(),
                    daily_used = %daily_used,
                    daily_limit = %config.daily_limit(),
                    monthly_used = %monthly_used,
                    monthly_limit = %config.monthly_limit(),
                    "Tavily account credits reserved"
                );
            }
            ReservationOutcome::Replay(outcome) => return replay_claim(outcome, operation),
            ReservationOutcome::InProgress => {
                return Err(denied(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ));
            }
            ReservationOutcome::Denied {
                denial,
                daily_used,
                monthly_used,
            } => {
                log_reconciliation(monthly_provider_usage, daily_used, monthly_used, config);
                log_denial(&denial, operation, Some(daily_used), Some(monthly_used));
                self.record_denial(call_id, request_fingerprint, operation, denial.clone(), now)
                    .await;
                return Err(TavilyCreditBudgetUseCasesError::Denied(denial));
            }
            ReservationOutcome::Conflict => {
                return Err(denied(
                    TavilyCreditBudgetReason::IdempotencyConflict,
                    TavilyCreditBudgetScope::Account,
                    None,
                ));
            }
        }

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
