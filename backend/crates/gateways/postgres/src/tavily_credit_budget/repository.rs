use async_trait::async_trait;
use chrono::{DateTime, Utc};
use core_application::tavily_credit_budget::{
    BootstrapCommand, BudgetReadiness, ExistingReservation, FinishFailureCommand,
    RecordDenialCommand, ReservationClaim, ReservationCommand, ReservationFailureState,
    ReservationOutcome, TavilyCreditBudgetConfig, TavilyCreditBudgetReason,
    TavilyCreditBudgetRepository, TavilyCreditBudgetRepositoryError, TavilyCreditBudgetScope,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::Value;
use uuid::Uuid;

use super::support::{
    capacity_denial, checked_add, checked_sub, database_error, denial, denied_outcome,
    insert_reservation, load_reservation, load_state, month_start, next_day_start,
    next_month_start, reconcile_periods, reconcile_usage, reservation_status, save_state,
};
use crate::transaction::transaction_ref;

#[derive(Clone, Default)]
pub struct PostgresTavilyCreditBudgetRepository;

impl PostgresTavilyCreditBudgetRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TavilyCreditBudgetRepository for PostgresTavilyCreditBudgetRepository {
    async fn readiness(
        &self,
        transaction: &UnitOfWorkTransaction,
        config: &TavilyCreditBudgetConfig,
    ) -> Result<BudgetReadiness, TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let state = load_state(transaction, false).await?;
        let Some(state) = state else {
            return Ok(BudgetReadiness::NotInitialized);
        };
        if state.account_id != config.account_id() {
            return Ok(BudgetReadiness::AccountMismatch);
        }
        if state.daily_limit != config.daily_limit()
            || state.monthly_limit != config.monthly_limit()
        {
            return Ok(BudgetReadiness::ConfigurationChanged);
        }
        Ok(BudgetReadiness::Ready)
    }

    async fn initialize(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: BootstrapCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        if let Some(state) = load_state(transaction, true).await? {
            return if state.account_id == command.config.account_id()
                && state.daily_limit == command.config.daily_limit()
                && state.monthly_limit == command.config.monthly_limit()
            {
                Ok(())
            } else {
                Err(TavilyCreditBudgetRepositoryError::InvalidState)
            };
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO tavily_credit_budget_state \
                 (singleton_key, account_id, daily_limit, monthly_limit, day_start, daily_used, daily_carry, day_provider_usage, daily_blocked_until, month_start, monthly_used, monthly_carry, provider_usage, updated_at) \
                 VALUES (1, $1, $2, $3, $4, 0, 0, $5, $6, $7, $5, 0, $5, $8)",
                [
                    command.config.account_id().to_owned().into(),
                    command.config.daily_limit().into(),
                    command.config.monthly_limit().into(),
                    command.day_start.into(),
                    command.monthly_provider_usage.into(),
                    command.daily_blocked_until.fixed_offset().into(),
                    command.month_start.into(),
                    command.now.fixed_offset().into(),
                ],
            ))
            .await
            .map_err(database_error)?;
        Ok(())
    }

    async fn find_reservation(
        &self,
        transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
    ) -> Result<ExistingReservation, TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let Some(row) = load_reservation(transaction, call_id, false).await? else {
            return Ok(ExistingReservation::Missing);
        };
        if row.request_fingerprint != request_fingerprint {
            return Ok(ExistingReservation::Conflict);
        }
        reservation_status(row)
    }

    async fn reserve(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: ReservationCommand,
    ) -> Result<ReservationOutcome, TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let Some(mut state) = load_state(transaction, true).await? else {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        };
        if state.account_id != command.config.account_id() {
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::AccountMismatch,
                    TavilyCreditBudgetScope::Account,
                    None,
                ),
                &state,
            ));
        }
        if state.daily_limit != command.config.daily_limit()
            || state.monthly_limit != command.config.monthly_limit()
        {
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::ConfigurationChanged,
                    TavilyCreditBudgetScope::Account,
                    None,
                ),
                &state,
            ));
        }

        if let Some(existing) = load_reservation(transaction, &command.call_id, true).await? {
            if existing.request_fingerprint != command.request_fingerprint
                || existing.operation != command.operation.as_str()
            {
                return Ok(ReservationOutcome::Conflict);
            }
            match existing.state.as_str() {
                "prepared" => {
                    if existing.day_start != command.day_start
                        || existing.month_start != command.month_start
                    {
                        let period = reconcile_periods(transaction, &mut state, &command).await?;
                        if let Some(denial) = period {
                            save_state(transaction, &state, command.now).await?;
                            return Ok(denied_outcome(denial, &state));
                        }
                        if let Some(denial) =
                            reconcile_usage(transaction, &mut state, &command).await?
                        {
                            save_state(transaction, &state, command.now).await?;
                            return Ok(denied_outcome(denial, &state));
                        }
                        if let Some(denial) = capacity_denial(
                            &state,
                            Decimal::ZERO,
                            command.day_start,
                            command.month_start,
                        ) {
                            save_state(transaction, &state, command.now).await?;
                            return Ok(denied_outcome(denial, &state));
                        }
                        if existing.day_start < command.day_start {
                            state.daily_carry =
                                checked_sub(state.daily_carry, existing.reserved_credits)?;
                        }
                        if existing.month_start < command.month_start {
                            state.monthly_carry =
                                checked_sub(state.monthly_carry, existing.reserved_credits)?;
                        }
                        {
                            transaction
                                .execute_raw(Statement::from_sql_and_values(
                                    DatabaseBackend::Postgres,
                                    "UPDATE tavily_credit_reservation SET day_start = $1, month_start = $2, updated_at = $3 WHERE call_id = $4 AND state = 'prepared'",
                                    [
                                        command.day_start.into(),
                                        command.month_start.into(),
                                        command.now.fixed_offset().into(),
                                        command.call_id.clone().into(),
                                    ],
                                ))
                                .await
                                .map_err(database_error)?;
                            save_state(transaction, &state, command.now).await?;
                            return Ok(ReservationOutcome::ReusedPrepared {
                                daily_used: state.daily_used,
                                monthly_used: state.monthly_used,
                            });
                        }
                    }
                    if let Some(denial) =
                        reconcile_periods(transaction, &mut state, &command).await?
                    {
                        save_state(transaction, &state, command.now).await?;
                        return Ok(denied_outcome(denial, &state));
                    }
                    if let Some(denial) = reconcile_usage(transaction, &mut state, &command).await?
                    {
                        save_state(transaction, &state, command.now).await?;
                        return Ok(denied_outcome(denial, &state));
                    }
                    if let Some(denial) = capacity_denial(
                        &state,
                        Decimal::ZERO,
                        command.day_start,
                        command.month_start,
                    ) {
                        save_state(transaction, &state, command.now).await?;
                        return Ok(denied_outcome(denial, &state));
                    }
                    save_state(transaction, &state, command.now).await?;
                    return Ok(ReservationOutcome::ReusedPrepared {
                        daily_used: state.daily_used,
                        monthly_used: state.monthly_used,
                    });
                }
                "dispatching" => return Ok(ReservationOutcome::InProgress),
                "settled" | "unknown" | "provider_rejected" | "denied" => {
                    return existing
                        .outcome_json
                        .map(ReservationOutcome::Replay)
                        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState);
                }
                _ => return Err(TavilyCreditBudgetRepositoryError::InvalidState),
            }
        }

        if let Some(denial) = reconcile_periods(transaction, &mut state, &command).await? {
            save_state(transaction, &state, command.now).await?;
            return Ok(denied_outcome(denial, &state));
        }
        if let Some(denial) = reconcile_usage(transaction, &mut state, &command).await? {
            save_state(transaction, &state, command.now).await?;
            return Ok(denied_outcome(denial, &state));
        }
        if state
            .daily_blocked_until
            .is_some_and(|until| until > command.now.fixed_offset())
        {
            let until = state
                .daily_blocked_until
                .map(|value| value.with_timezone(&Utc));
            save_state(transaction, &state, command.now).await?;
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::PeriodResetUnconfirmed,
                    TavilyCreditBudgetScope::Daily,
                    until,
                ),
                &state,
            ));
        }

        let reservation = command.operation.reservation();
        if state.daily_used + reservation > state.daily_limit {
            let next_reset =
                next_day_start(command.day_start).map(|value| value.with_timezone(&Utc));
            save_state(transaction, &state, command.now).await?;
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    next_reset,
                ),
                &state,
            ));
        }
        if state.monthly_used + reservation > state.monthly_limit {
            let next_reset =
                next_month_start(command.month_start).map(|value| value.with_timezone(&Utc));
            save_state(transaction, &state, command.now).await?;
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::MonthlyLimitExceeded,
                    TavilyCreditBudgetScope::Monthly,
                    next_reset,
                ),
                &state,
            ));
        }

        state.daily_used += reservation;
        state.monthly_used += reservation;
        insert_reservation(transaction, &command, "prepared", reservation, None).await?;
        save_state(transaction, &state, command.now).await?;
        Ok(ReservationOutcome::Reserved {
            daily_used: state.daily_used,
            monthly_used: state.monthly_used,
        })
    }

    async fn record_denial(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: RecordDenialCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO tavily_credit_reservation \
                 (reservation_id, call_id, request_fingerprint, operation, state, reserved_credits, settled_credits, day_start, month_start, outcome_json, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, 'denied', 0, 0, $5, $6, $7, $8, $8) \
                 ON CONFLICT (call_id) DO NOTHING",
                [
                    Uuid::new_v4().into(),
                    command.call_id.into(),
                    command.request_fingerprint.into(),
                    command.operation.as_str().into(),
                    command.now.date_naive().into(),
                    month_start(command.now.date_naive()).into(),
                    command.outcome.into(),
                    command.now.fixed_offset().into(),
                ],
            ))
            .await
            .map_err(database_error)?;
        Ok(())
    }

    async fn claim(
        &self,
        transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
        now: DateTime<Utc>,
    ) -> Result<ReservationClaim, TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let Some(row) = load_reservation(transaction, call_id, true).await? else {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        };
        if row.request_fingerprint != request_fingerprint {
            return Ok(ReservationClaim::Conflict);
        }
        match row.state.as_str() {
            "prepared" => {
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        DatabaseBackend::Postgres,
                        "UPDATE tavily_credit_reservation SET state = 'dispatching', updated_at = $1 WHERE call_id = $2 AND state = 'prepared'",
                        [now.fixed_offset().into(), call_id.to_owned().into()],
                    ))
                    .await
                    .map_err(database_error)?;
                Ok(ReservationClaim::Claimed)
            }
            "dispatching" => Ok(ReservationClaim::InProgress),
            "settled" | "unknown" | "provider_rejected" | "denied" => row
                .outcome_json
                .map(ReservationClaim::Replay)
                .ok_or(TavilyCreditBudgetRepositoryError::InvalidState),
            _ => Err(TavilyCreditBudgetRepositoryError::InvalidState),
        }
    }

    async fn settle(
        &self,
        transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
        actual_credits: Decimal,
        outcome: Value,
        now: DateTime<Utc>,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let Some(row) = load_reservation(transaction, call_id, true).await? else {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        };
        if row.request_fingerprint != request_fingerprint {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        if row.state == "settled" {
            return if row.settled_credits == Some(actual_credits) {
                Ok(())
            } else {
                Err(TavilyCreditBudgetRepositoryError::InvalidState)
            };
        }
        if row.state != "dispatching" {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        let mut state = load_state(transaction, true)
            .await?
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let adjustment = actual_credits - row.reserved_credits;
        state.daily_used = checked_add(state.daily_used, adjustment)?;
        state.monthly_used = checked_add(state.monthly_used, adjustment)?;
        if row.day_start < state.day_start {
            state.daily_carry = checked_add(state.daily_carry, adjustment)?;
        }
        if row.month_start < state.month_start {
            state.monthly_carry = checked_add(state.monthly_carry, adjustment)?;
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE tavily_credit_reservation SET state = 'settled', settled_credits = $1, outcome_json = $2, updated_at = $3 WHERE call_id = $4",
                [
                    actual_credits.into(),
                    outcome.into(),
                    now.fixed_offset().into(),
                    call_id.to_owned().into(),
                ],
            ))
            .await
            .map_err(database_error)?;
        save_state(transaction, &state, now).await?;
        Ok(())
    }

    async fn finish_failure(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: FinishFailureCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let transaction =
            transaction_ref(transaction).ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        let state = match command.state {
            ReservationFailureState::Unknown => "unknown",
            ReservationFailureState::ProviderRejected => "provider_rejected",
        };
        let result = transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE tavily_credit_reservation SET state = $1, outcome_json = $2, updated_at = $3 \
                 WHERE call_id = $4 AND request_fingerprint = $5 AND state = 'dispatching'",
                [
                    state.into(),
                    command.outcome.into(),
                    command.now.fixed_offset().into(),
                    command.call_id.into(),
                    command.request_fingerprint.into(),
                ],
            ))
            .await
            .map_err(database_error)?;
        if result.rows_affected() != 1 {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        Ok(())
    }
}
