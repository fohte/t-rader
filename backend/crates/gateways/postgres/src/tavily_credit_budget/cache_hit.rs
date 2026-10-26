use chrono::Datelike;
use core_application::tavily_credit_budget::{
    RecordCacheHitCommand, ReservationClaim, TavilyCreditBudgetRepositoryError,
};
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, Statement};
use uuid::Uuid;

use super::support::{checked_sub, database_error, load_reservation, load_state, save_state};

pub(super) async fn record_cache_hit(
    transaction: &DatabaseTransaction,
    command: RecordCacheHitCommand,
) -> Result<ReservationClaim, TavilyCreditBudgetRepositoryError> {
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO tavily_credit_reservation \
             (reservation_id, call_id, request_fingerprint, operation, state, reserved_credits, settled_credits, day_start, month_start, outcome_json, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, 'settled', 0, 0, $5, $6, $7, $8, $8) \
             ON CONFLICT (call_id) DO NOTHING",
            [
                Uuid::new_v4().into(),
                command.call_id.clone().into(),
                command.request_fingerprint.clone().into(),
                command.operation.as_str().into(),
                command.now.date_naive().into(),
                command
                    .now
                    .date_naive()
                    .with_day(1)
                    .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?
                    .into(),
                command.outcome.clone().into(),
                command.now.fixed_offset().into(),
            ],
        ))
        .await
        .map_err(database_error)?;

    let Some(row) = load_reservation(transaction, &command.call_id, true).await? else {
        return Err(TavilyCreditBudgetRepositoryError::InvalidState);
    };
    if row.request_fingerprint != command.request_fingerprint
        || row.operation != command.operation.as_str()
    {
        return Ok(ReservationClaim::Conflict);
    }

    match row.state.as_str() {
        "settled" | "unknown" | "provider_rejected" | "denied" => row
            .outcome_json
            .map(ReservationClaim::Replay)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState),
        "dispatching" => Ok(ReservationClaim::InProgress),
        "prepared" => {
            release_prepared_reservation(
                transaction,
                &command,
                row.reserved_credits,
                row.day_start,
                row.month_start,
            )
            .await?;
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "UPDATE tavily_credit_reservation SET state = 'settled', reserved_credits = 0, settled_credits = 0, outcome_json = $1, updated_at = $2 WHERE call_id = $3 AND state = 'prepared'",
                    [
                        command.outcome.clone().into(),
                        command.now.fixed_offset().into(),
                        command.call_id.into(),
                    ],
                ))
                .await
                .map_err(database_error)?;
            Ok(ReservationClaim::Replay(command.outcome))
        }
        _ => Err(TavilyCreditBudgetRepositoryError::InvalidState),
    }
}

async fn release_prepared_reservation(
    transaction: &DatabaseTransaction,
    command: &RecordCacheHitCommand,
    reserved_credits: Decimal,
    reservation_day: chrono::NaiveDate,
    reservation_month: chrono::NaiveDate,
) -> Result<(), TavilyCreditBudgetRepositoryError> {
    let mut state = load_state(transaction, true)
        .await?
        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
    if reservation_day > state.day_start || reservation_month > state.month_start {
        return Err(TavilyCreditBudgetRepositoryError::InvalidState);
    }
    state.daily_used = checked_sub(state.daily_used, reserved_credits)?;
    if reservation_day < state.day_start {
        state.daily_carry = checked_sub(state.daily_carry, reserved_credits)?;
    }
    state.monthly_used = checked_sub(state.monthly_used, reserved_credits)?;
    if reservation_month < state.month_start {
        state.monthly_carry = checked_sub(state.monthly_carry, reserved_credits)?;
    }
    save_state(transaction, &state, command.now).await
}
