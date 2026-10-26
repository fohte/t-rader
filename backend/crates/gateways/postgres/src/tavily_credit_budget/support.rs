use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, TimeZone, Utc};
use core_application::tavily_credit_budget::{
    ExistingReservation, ReservationCommand, ReservationOutcome, TavilyCreditBudgetDenial,
    TavilyCreditBudgetReason, TavilyCreditBudgetRepositoryError, TavilyCreditBudgetScope,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::Value;
use uuid::Uuid;

const STATE_COLUMNS: &str = "account_id, daily_limit, monthly_limit, day_start, daily_used, daily_carry, day_provider_usage, daily_blocked_until, month_start, monthly_used, monthly_carry, provider_usage";
const RESERVATION_COLUMNS: &str = "request_fingerprint, operation, state, reserved_credits, settled_credits, day_start, month_start, outcome_json";

#[derive(Debug)]
pub(super) struct BudgetState {
    pub(super) account_id: String,
    pub(super) daily_limit: Decimal,
    pub(super) monthly_limit: Decimal,
    pub(super) day_start: NaiveDate,
    pub(super) daily_used: Decimal,
    pub(super) daily_carry: Decimal,
    pub(super) day_provider_usage: Decimal,
    pub(super) daily_blocked_until: Option<DateTime<FixedOffset>>,
    pub(super) month_start: NaiveDate,
    pub(super) monthly_used: Decimal,
    pub(super) monthly_carry: Decimal,
    pub(super) provider_usage: Decimal,
}

#[derive(Debug)]
pub(super) struct ReservationRow {
    pub(super) request_fingerprint: String,
    pub(super) operation: String,
    pub(super) state: String,
    pub(super) reserved_credits: Decimal,
    pub(super) settled_credits: Option<Decimal>,
    pub(super) day_start: NaiveDate,
    pub(super) month_start: NaiveDate,
    pub(super) outcome_json: Option<Value>,
}

pub(super) async fn load_state<C: ConnectionTrait>(
    connection: &C,
    lock: bool,
) -> Result<Option<BudgetState>, TavilyCreditBudgetRepositoryError> {
    let lock_clause = if lock { " FOR UPDATE" } else { "" };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!("SELECT {STATE_COLUMNS} FROM tavily_credit_budget_state WHERE singleton_key = 1{lock_clause}"),
            [],
        ))
        .await
        .map_err(database_error)?;
    row.map(|row| {
        Ok(BudgetState {
            account_id: row.try_get("", "account_id").map_err(database_error)?,
            daily_limit: row.try_get("", "daily_limit").map_err(database_error)?,
            monthly_limit: row.try_get("", "monthly_limit").map_err(database_error)?,
            day_start: row.try_get("", "day_start").map_err(database_error)?,
            daily_used: row.try_get("", "daily_used").map_err(database_error)?,
            daily_carry: row.try_get("", "daily_carry").map_err(database_error)?,
            day_provider_usage: row
                .try_get("", "day_provider_usage")
                .map_err(database_error)?,
            daily_blocked_until: row
                .try_get("", "daily_blocked_until")
                .map_err(database_error)?,
            month_start: row.try_get("", "month_start").map_err(database_error)?,
            monthly_used: row.try_get("", "monthly_used").map_err(database_error)?,
            monthly_carry: row.try_get("", "monthly_carry").map_err(database_error)?,
            provider_usage: row.try_get("", "provider_usage").map_err(database_error)?,
        })
    })
    .transpose()
}

pub(super) async fn save_state<C: ConnectionTrait>(
    connection: &C,
    state: &BudgetState,
    now: DateTime<Utc>,
) -> Result<(), TavilyCreditBudgetRepositoryError> {
    connection
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE tavily_credit_budget_state SET day_start = $1, daily_used = $2, daily_carry = $3, day_provider_usage = $4, \
             daily_blocked_until = $5, month_start = $6, monthly_used = $7, monthly_carry = $8, provider_usage = $9, updated_at = $10 \
             WHERE singleton_key = 1",
            [
                state.day_start.into(),
                state.daily_used.into(),
                state.daily_carry.into(),
                state.day_provider_usage.into(),
                state.daily_blocked_until.into(),
                state.month_start.into(),
                state.monthly_used.into(),
                state.monthly_carry.into(),
                state.provider_usage.into(),
                now.fixed_offset().into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
}

pub(super) async fn load_reservation<C: ConnectionTrait>(
    connection: &C,
    call_id: &str,
    lock: bool,
) -> Result<Option<ReservationRow>, TavilyCreditBudgetRepositoryError> {
    let lock_clause = if lock { " FOR UPDATE" } else { "" };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!("SELECT {RESERVATION_COLUMNS} FROM tavily_credit_reservation WHERE call_id = $1{lock_clause}"),
            [call_id.to_owned().into()],
        ))
        .await
        .map_err(database_error)?;
    row.map(|row| {
        Ok(ReservationRow {
            request_fingerprint: row
                .try_get("", "request_fingerprint")
                .map_err(database_error)?,
            operation: row.try_get("", "operation").map_err(database_error)?,
            state: row.try_get("", "state").map_err(database_error)?,
            reserved_credits: row
                .try_get("", "reserved_credits")
                .map_err(database_error)?,
            settled_credits: row.try_get("", "settled_credits").map_err(database_error)?,
            day_start: row.try_get("", "day_start").map_err(database_error)?,
            month_start: row.try_get("", "month_start").map_err(database_error)?,
            outcome_json: row.try_get("", "outcome_json").map_err(database_error)?,
        })
    })
    .transpose()
}

pub(super) fn reservation_status(
    row: ReservationRow,
) -> Result<ExistingReservation, TavilyCreditBudgetRepositoryError> {
    match row.state.as_str() {
        "prepared" => Ok(ExistingReservation::Prepared),
        "dispatching" => Ok(ExistingReservation::InProgress),
        "settled" | "unknown" | "provider_rejected" | "denied" => row
            .outcome_json
            .map(ExistingReservation::Replay)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState),
        _ => Err(TavilyCreditBudgetRepositoryError::InvalidState),
    }
}

pub(super) async fn insert_reservation<C: ConnectionTrait>(
    connection: &C,
    command: &ReservationCommand,
    state: &str,
    reserved: Decimal,
    outcome: Option<Value>,
) -> Result<(), TavilyCreditBudgetRepositoryError> {
    connection
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO tavily_credit_reservation \
             (reservation_id, call_id, request_fingerprint, operation, state, reserved_credits, settled_credits, day_start, month_start, outcome_json, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, NULL, $7, $8, $9, $10, $10)",
            [
                Uuid::new_v4().into(),
                command.call_id.clone().into(),
                command.request_fingerprint.clone().into(),
                command.operation.as_str().into(),
                state.into(),
                reserved.into(),
                command.day_start.into(),
                command.month_start.into(),
                outcome.into(),
                command.now.fixed_offset().into(),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
}

pub(super) async fn reconcile_periods<C: ConnectionTrait>(
    connection: &C,
    state: &mut BudgetState,
    command: &ReservationCommand,
) -> Result<Option<TavilyCreditBudgetDenial>, TavilyCreditBudgetRepositoryError> {
    if command.month_start < state.month_start || command.day_start < state.day_start {
        return Ok(Some(denial(
            TavilyCreditBudgetReason::PeriodResetUnconfirmed,
            TavilyCreditBudgetScope::Confirmation,
            None,
        )));
    }
    let month_changed = command.month_start > state.month_start;
    if month_changed {
        let reset_observed = command.monthly_provider_usage < state.provider_usage
            || (state.provider_usage == Decimal::ZERO
                && command.monthly_provider_usage == Decimal::ZERO);
        if !reset_observed {
            return Ok(Some(denial(
                TavilyCreditBudgetReason::PeriodResetUnconfirmed,
                TavilyCreditBudgetScope::Monthly,
                next_month_start(state.month_start).map(|value| value.with_timezone(&Utc)),
            )));
        }
        let carry = carry_credits(connection, "month_start", command.month_start).await?;
        state.month_start = command.month_start;
        state.monthly_used = checked_add(command.monthly_provider_usage, carry)?;
        state.monthly_carry = carry;
        state.provider_usage = command.monthly_provider_usage;
    } else if command.monthly_provider_usage < state.provider_usage {
        return Ok(Some(denial(
            TavilyCreditBudgetReason::PeriodResetUnconfirmed,
            TavilyCreditBudgetScope::Monthly,
            next_month_start(state.month_start).map(|value| value.with_timezone(&Utc)),
        )));
    }

    if command.day_start > state.day_start {
        let pending_carry = carry_credits(connection, "day_start", command.day_start).await?;
        let unaccounted_provider_usage = if month_changed {
            command.monthly_provider_usage
        } else {
            let observed_since_day_start = command
                .monthly_provider_usage
                .checked_sub(state.day_provider_usage)
                .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
            observed_since_day_start
                .checked_sub(state.daily_used)
                .unwrap_or(Decimal::ZERO)
                .max(Decimal::ZERO)
        };
        let carry = checked_add(pending_carry, unaccounted_provider_usage)?;
        state.day_start = command.day_start;
        state.daily_used = carry;
        state.daily_carry = carry;
        state.day_provider_usage = command.monthly_provider_usage;
        state.daily_blocked_until = None;
    }
    Ok(None)
}

pub(super) async fn reconcile_usage<C: ConnectionTrait>(
    connection: &C,
    state: &mut BudgetState,
    command: &ReservationCommand,
) -> Result<Option<TavilyCreditBudgetDenial>, TavilyCreditBudgetRepositoryError> {
    if command.monthly_provider_usage < state.provider_usage {
        return Ok(Some(denial(
            TavilyCreditBudgetReason::PeriodResetUnconfirmed,
            TavilyCreditBudgetScope::Monthly,
            next_month_start(state.month_start).map(|value| value.with_timezone(&Utc)),
        )));
    }
    let in_flight_monthly = sum_reservations(
        connection,
        "month_start",
        state.month_start,
        &["prepared", "dispatching", "unknown", "provider_rejected"],
    )
    .await?;
    let provider_upper_bound = checked_add(command.monthly_provider_usage, in_flight_monthly)?;
    let tracked_current_month = state
        .monthly_used
        .checked_sub(state.monthly_carry)
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
    state.monthly_used = checked_add(
        state.monthly_carry,
        tracked_current_month.max(provider_upper_bound),
    )?;

    let daily_provider_delta = command
        .monthly_provider_usage
        .checked_sub(state.day_provider_usage)
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
    let in_flight_daily = sum_reservations(
        connection,
        "day_start",
        state.day_start,
        &["prepared", "dispatching", "unknown", "provider_rejected"],
    )
    .await?;
    let provider_daily_upper_bound = checked_add(daily_provider_delta, in_flight_daily)?;
    let tracked_current_day = state
        .daily_used
        .checked_sub(state.daily_carry)
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
    state.daily_used = checked_add(
        state.daily_carry,
        tracked_current_day.max(provider_daily_upper_bound),
    )?;
    state.provider_usage = command.monthly_provider_usage;
    Ok(None)
}

pub(super) async fn carry_credits<C: ConnectionTrait>(
    connection: &C,
    period_column: &str,
    period_start: NaiveDate,
) -> Result<Decimal, TavilyCreditBudgetRepositoryError> {
    let sql = format!(
        "SELECT COALESCE(SUM(CASE WHEN state = 'settled' THEN settled_credits ELSE reserved_credits END), 0)::numeric AS credits \
         FROM tavily_credit_reservation WHERE {period_column} < $1 AND \
         (state IN ('prepared', 'dispatching', 'unknown', 'provider_rejected') OR \
          (state = 'settled' AND updated_at >= ($1::date::timestamp AT TIME ZONE 'UTC')))"
    );
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [period_start.into()],
        ))
        .await
        .map_err(database_error)?
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
    row.try_get("", "credits").map_err(database_error)
}

pub(super) fn capacity_denial(
    state: &BudgetState,
    additional: Decimal,
    day_start: NaiveDate,
    month_start: NaiveDate,
) -> Option<TavilyCreditBudgetDenial> {
    if state.daily_used + additional > state.daily_limit {
        return Some(denial(
            TavilyCreditBudgetReason::DailyLimitExceeded,
            TavilyCreditBudgetScope::Daily,
            next_day_start(day_start),
        ));
    }
    if state.monthly_used + additional > state.monthly_limit {
        return Some(denial(
            TavilyCreditBudgetReason::MonthlyLimitExceeded,
            TavilyCreditBudgetScope::Monthly,
            next_month_start(month_start),
        ));
    }
    None
}

pub(super) async fn sum_reservations<C: ConnectionTrait>(
    connection: &C,
    period_column: &str,
    period_start: NaiveDate,
    states: &[&str],
) -> Result<Decimal, TavilyCreditBudgetRepositoryError> {
    let states = states
        .iter()
        .map(|state| format!("'{state}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COALESCE(SUM(reserved_credits), 0)::numeric AS credits FROM tavily_credit_reservation \
         WHERE {period_column} = $1 AND state IN ({states})"
    );
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            [period_start.into()],
        ))
        .await
        .map_err(database_error)?
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
    row.try_get("", "credits").map_err(database_error)
}

pub(super) fn next_day_start(day: NaiveDate) -> Option<DateTime<Utc>> {
    let next_day = day.succ_opt()?;
    Some(Utc.from_utc_datetime(&next_day.and_hms_opt(0, 0, 0)?))
}

pub(super) fn next_month_start(day: NaiveDate) -> Option<DateTime<Utc>> {
    let next_month = if day.month() == 12 {
        NaiveDate::from_ymd_opt(day.year().checked_add(1)?, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(day.year(), day.month() + 1, 1)?
    };
    Some(Utc.from_utc_datetime(&next_month.and_hms_opt(0, 0, 0)?))
}

pub(super) fn month_start(day: NaiveDate) -> Option<NaiveDate> {
    day.with_day(1)
}

pub(super) fn checked_add(
    left: Decimal,
    right: Decimal,
) -> Result<Decimal, TavilyCreditBudgetRepositoryError> {
    left.checked_add(right)
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)
}

pub(super) fn checked_sub(
    left: Decimal,
    right: Decimal,
) -> Result<Decimal, TavilyCreditBudgetRepositoryError> {
    left.checked_sub(right)
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)
}

pub(super) fn denial(
    reason: TavilyCreditBudgetReason,
    scope: TavilyCreditBudgetScope,
    next_reset: Option<DateTime<Utc>>,
) -> TavilyCreditBudgetDenial {
    TavilyCreditBudgetDenial::new(reason, scope, next_reset)
}

pub(super) fn denied_outcome(
    denial: TavilyCreditBudgetDenial,
    state: &BudgetState,
) -> ReservationOutcome {
    ReservationOutcome::Denied {
        denial,
        daily_used: state.daily_used,
        monthly_used: state.monthly_used,
    }
}

pub(super) fn database_error(_: sea_orm::DbErr) -> TavilyCreditBudgetRepositoryError {
    TavilyCreditBudgetRepositoryError::Database
}
