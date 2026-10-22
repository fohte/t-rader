use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use core_application::{
    execution_budget::{
        BudgetReservation, BudgetReservationCommand, BudgetReservationResult,
        BudgetReservationState, BudgetReservationTransition, BudgetResource,
        ExecutionBudgetRepository, ExecutionBudgetRepositoryError,
    },
    unit_of_work::UnitOfWorkTransaction,
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

use crate::{persistence::persistence_error, transaction::transaction_ref};

#[derive(Clone, Default)]
pub struct PostgresExecutionBudgetRepository;

impl PostgresExecutionBudgetRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExecutionBudgetRepository for PostgresExecutionBudgetRepository {
    async fn initialize_task(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        deadline_at: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(ExecutionBudgetRepositoryError::InvalidTransaction)?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO task_execution_state (task_id, deadline_at, usage_complete, created_at, updated_at) \
                 VALUES ($1, $2, true, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) \
                 ON CONFLICT (task_id) DO NOTHING",
                [task_id.into(), deadline_at.into()],
            ))
            .await
            .map_err(database_error)?;
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT deadline_at, usage_complete FROM task_execution_state WHERE task_id = $1",
                [task_id.into()],
            ))
            .await
            .map_err(database_error)?
            .ok_or(ExecutionBudgetRepositoryError::StateNotFound)?;
        let stored_deadline = row
            .try_get::<DateTime<FixedOffset>>("", "deadline_at")
            .map_err(database_error)?;
        let usage_complete = row
            .try_get::<bool>("", "usage_complete")
            .map_err(database_error)?;
        if !usage_complete {
            return Err(ExecutionBudgetRepositoryError::UsageNotMeasured);
        }
        if stored_deadline.timestamp_micros() != deadline_at.timestamp_micros() {
            return Err(ExecutionBudgetRepositoryError::ReservationConflict);
        }
        Ok(())
    }

    async fn reserve(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: BudgetReservationCommand,
    ) -> Result<BudgetReservationResult, ExecutionBudgetRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(ExecutionBudgetRepositoryError::InvalidTransaction)?;
        let state = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT deadline_at, usage_complete FROM task_execution_state WHERE task_id = $1 FOR UPDATE",
                [command.task_id.into()],
            ))
            .await
            .map_err(database_error)?
            .ok_or(ExecutionBudgetRepositoryError::StateNotFound)?;
        let deadline_at = state
            .try_get::<DateTime<FixedOffset>>("", "deadline_at")
            .map_err(database_error)?;
        if !state
            .try_get::<bool>("", "usage_complete")
            .map_err(database_error)?
        {
            return Err(ExecutionBudgetRepositoryError::UsageNotMeasured);
        }
        if deadline_at <= command.now {
            return Err(ExecutionBudgetRepositoryError::DeadlineExpired);
        }
        let amount = common_amount(&command)?;

        let existing = transaction
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT reservation_id, task_id, call_id, request_fingerprint, resource, resource_key, scope_key, amount, state, response_json, created_at, updated_at \
                 FROM execution_reservation WHERE task_id = $1 AND call_id = $2 ORDER BY resource_key, scope_key",
                [command.task_id.into(), command.call_id.clone().into()],
            ))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(reservation_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        if !existing.is_empty() {
            if existing.iter().any(|reservation| {
                reservation.state == BudgetReservationState::Released
                    || reservation.resource != command.resource
                    || reservation.request_fingerprint != command.request_fingerprint
            }) || !same_charges(&existing, &command)
            {
                return Err(ExecutionBudgetRepositoryError::ReservationConflict);
            }
            return Ok(BudgetReservationResult {
                completed_response: common_response(&existing)?,
                reservations: existing,
            });
        }

        for charge in &command.charges {
            let row = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "SELECT COALESCE(SUM(amount), 0)::BIGINT AS used \
                     FROM execution_reservation \
                     WHERE task_id = $1 AND resource = $2 AND resource_key = $3 AND scope_key = $4 \
                       AND state IN ('reserved', 'consumed')",
                    [
                        command.task_id.into(),
                        command.resource.as_str().into(),
                        charge.resource_key.clone().into(),
                        charge.scope_key.clone().into(),
                    ],
                ))
                .await
                .map_err(database_error)?
                .ok_or(ExecutionBudgetRepositoryError::StateNotFound)?;
            let used = row.try_get::<i64>("", "used").map_err(database_error)?;
            if charge.limit.is_some_and(|limit| {
                used.saturating_add(i64::try_from(charge.amount).unwrap_or(i64::MAX))
                    > i64::try_from(limit).unwrap_or(i64::MAX)
            }) {
                return Err(ExecutionBudgetRepositoryError::LimitExceeded {
                    resource_key: charge.resource_key.clone(),
                });
            }
        }

        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                counter_update_sql(command.resource),
                [amount.into(), command.task_id.into(), command.now.into()],
            ))
            .await
            .map_err(database_error)?;

        let mut reservations = Vec::with_capacity(command.charges.len());
        for charge in &command.charges {
            let id = Uuid::new_v4();
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "INSERT INTO execution_reservation \
                     (reservation_id, task_id, call_id, request_fingerprint, resource, resource_key, scope_key, amount, state, response_json, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'reserved', NULL, $9, $9)",
                    [
                        id.into(),
                        command.task_id.into(),
                        command.call_id.clone().into(),
                        command.request_fingerprint.clone().into(),
                        command.resource.as_str().into(),
                        charge.resource_key.clone().into(),
                        charge.scope_key.clone().into(),
                        i64::try_from(charge.amount).map_err(|_| ExecutionBudgetRepositoryError::ReservationConflict)?.into(),
                        command.now.into(),
                    ],
                ))
                .await
                .map_err(database_error)?;
            reservations.push(BudgetReservation {
                reservation_id: id,
                task_id: command.task_id,
                call_id: command.call_id.clone(),
                request_fingerprint: command.request_fingerprint.clone(),
                resource: command.resource,
                resource_key: charge.resource_key.clone(),
                scope_key: charge.scope_key.clone(),
                amount: charge.amount,
                state: BudgetReservationState::Reserved,
                completed_response: None,
                created_at: command.now,
                updated_at: command.now,
            });
        }
        Ok(BudgetReservationResult {
            reservations,
            completed_response: None,
        })
    }

    async fn transition_call(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        call_id: &str,
        state: BudgetReservationState,
        now: DateTime<FixedOffset>,
    ) -> Result<BudgetReservationTransition, ExecutionBudgetRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(ExecutionBudgetRepositoryError::InvalidTransaction)?;
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT reservation_id, task_id, call_id, request_fingerprint, resource, resource_key, scope_key, amount, state, response_json, created_at, updated_at \
                 FROM execution_reservation WHERE task_id = $1 AND call_id = $2 ORDER BY resource_key, scope_key FOR UPDATE",
                [task_id.into(), call_id.to_string().into()],
            ))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(reservation_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty() {
            return Err(ExecutionBudgetRepositoryError::InvalidReservationState);
        }
        if rows.iter().all(|reservation| reservation.state == state) {
            return Ok(BudgetReservationTransition {
                reservations: rows,
                transitioned: false,
            });
        }
        if rows
            .iter()
            .any(|reservation| reservation.state != BudgetReservationState::Reserved)
        {
            return Err(ExecutionBudgetRepositoryError::InvalidReservationState);
        }
        if state == BudgetReservationState::Released {
            let amount = rows[0].amount;
            if rows.iter().any(|reservation| reservation.amount != amount) {
                return Err(ExecutionBudgetRepositoryError::ReservationConflict);
            }
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    counter_decrement_sql(rows[0].resource),
                    [
                        i64::try_from(amount)
                            .map_err(|_| ExecutionBudgetRepositoryError::ReservationConflict)?
                            .into(),
                        task_id.into(),
                        now.into(),
                    ],
                ))
                .await
                .map_err(database_error)?;
        }
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE execution_reservation SET state = $3, updated_at = $4 WHERE task_id = $1 AND call_id = $2 AND state = 'reserved'",
                [task_id.into(), call_id.to_string().into(), state.as_str().into(), now.into()],
            ))
            .await
            .map_err(database_error)?;
        Ok(BudgetReservationTransition {
            reservations: rows
                .into_iter()
                .map(|mut reservation| {
                    reservation.state = state;
                    reservation.updated_at = now;
                    reservation
                })
                .collect(),
            transitioned: true,
        })
    }

    async fn complete_call(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        call_id: &str,
        response: serde_json::Value,
        now: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError> {
        let transaction = transaction_ref(transaction)
            .ok_or(ExecutionBudgetRepositoryError::InvalidTransaction)?;
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT reservation_id, task_id, call_id, request_fingerprint, resource, resource_key, scope_key, amount, state, response_json, created_at, updated_at \
                 FROM execution_reservation WHERE task_id = $1 AND call_id = $2 ORDER BY resource_key, scope_key FOR UPDATE",
                [task_id.into(), call_id.to_string().into()],
            ))
            .await
            .map_err(database_error)?
            .into_iter()
            .map(reservation_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty()
            || rows
                .iter()
                .any(|row| row.state != BudgetReservationState::Consumed)
        {
            return Err(ExecutionBudgetRepositoryError::InvalidReservationState);
        }
        if rows
            .iter()
            .all(|row| row.completed_response.as_ref() == Some(&response))
        {
            return Ok(());
        }
        if rows.iter().any(|row| row.completed_response.is_some()) {
            return Err(ExecutionBudgetRepositoryError::ReservationConflict);
        }
        let result = transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE execution_reservation SET response_json = $3, updated_at = $4 WHERE task_id = $1 AND call_id = $2 AND state = 'consumed' AND response_json IS NULL",
                [task_id.into(), call_id.to_string().into(), response.into(), now.into()],
            ))
            .await
            .map_err(database_error)?;
        if result.rows_affected()
            != u64::try_from(rows.len())
                .map_err(|_| ExecutionBudgetRepositoryError::ReservationConflict)?
        {
            return Err(ExecutionBudgetRepositoryError::ReservationConflict);
        }
        Ok(())
    }
}

fn common_amount(
    command: &BudgetReservationCommand,
) -> Result<i64, ExecutionBudgetRepositoryError> {
    let Some(first) = command.charges.first() else {
        return Err(ExecutionBudgetRepositoryError::ReservationConflict);
    };
    if command
        .charges
        .iter()
        .any(|charge| charge.amount != first.amount)
    {
        return Err(ExecutionBudgetRepositoryError::ReservationConflict);
    }
    i64::try_from(first.amount).map_err(|_| ExecutionBudgetRepositoryError::ReservationConflict)
}

fn same_charges(rows: &[BudgetReservation], command: &BudgetReservationCommand) -> bool {
    rows.len() == command.charges.len()
        && command.charges.iter().all(|charge| {
            rows.iter().any(|row| {
                row.resource_key == charge.resource_key
                    && row.scope_key == charge.scope_key
                    && row.amount == charge.amount
            })
        })
}

fn common_response(
    rows: &[BudgetReservation],
) -> Result<Option<serde_json::Value>, ExecutionBudgetRepositoryError> {
    let response = rows.first().and_then(|row| row.completed_response.clone());
    if rows.iter().any(|row| row.completed_response != response) {
        return Err(ExecutionBudgetRepositoryError::ReservationConflict);
    }
    Ok(response)
}

fn reservation_from_row(
    row: sea_orm::QueryResult,
) -> Result<BudgetReservation, ExecutionBudgetRepositoryError> {
    let resource_text = row
        .try_get::<String>("", "resource")
        .map_err(database_error)?;
    let state_text = row.try_get::<String>("", "state").map_err(database_error)?;
    Ok(BudgetReservation {
        reservation_id: row.try_get("", "reservation_id").map_err(database_error)?,
        task_id: row.try_get("", "task_id").map_err(database_error)?,
        call_id: row.try_get("", "call_id").map_err(database_error)?,
        request_fingerprint: row
            .try_get("", "request_fingerprint")
            .map_err(database_error)?,
        resource: BudgetResource::parse(&resource_text)
            .ok_or(ExecutionBudgetRepositoryError::ReservationConflict)?,
        resource_key: row.try_get("", "resource_key").map_err(database_error)?,
        scope_key: row.try_get("", "scope_key").map_err(database_error)?,
        amount: u64::try_from(row.try_get::<i64>("", "amount").map_err(database_error)?)
            .map_err(|_| ExecutionBudgetRepositoryError::ReservationConflict)?,
        state: BudgetReservationState::parse(&state_text)
            .ok_or(ExecutionBudgetRepositoryError::ReservationConflict)?,
        completed_response: row.try_get("", "response_json").map_err(database_error)?,
        created_at: row.try_get("", "created_at").map_err(database_error)?,
        updated_at: row.try_get("", "updated_at").map_err(database_error)?,
    })
}

fn counter_update_sql(resource: BudgetResource) -> &'static str {
    match resource {
        BudgetResource::ModelCall => {
            "UPDATE task_execution_state SET model_calls_used = model_calls_used + $1, updated_at = $3 WHERE task_id = $2"
        }
        BudgetResource::ToolCall => {
            "UPDATE task_execution_state SET tool_calls_used = tool_calls_used + $1, updated_at = $3 WHERE task_id = $2"
        }
        BudgetResource::SubagentRun => {
            "UPDATE task_execution_state SET subagent_runs_used = subagent_runs_used + $1, updated_at = $3 WHERE task_id = $2"
        }
    }
}

fn counter_decrement_sql(resource: BudgetResource) -> &'static str {
    match resource {
        BudgetResource::ModelCall => {
            "UPDATE task_execution_state SET model_calls_used = model_calls_used - $1, updated_at = $3 WHERE task_id = $2"
        }
        BudgetResource::ToolCall => {
            "UPDATE task_execution_state SET tool_calls_used = tool_calls_used - $1, updated_at = $3 WHERE task_id = $2"
        }
        BudgetResource::SubagentRun => {
            "UPDATE task_execution_state SET subagent_runs_used = subagent_runs_used - $1, updated_at = $3 WHERE task_id = $2"
        }
    }
}

fn database_error(error: sea_orm::DbErr) -> ExecutionBudgetRepositoryError {
    ExecutionBudgetRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, SubsecRound, Utc};
    use core_application::{
        execution_budget::{
            BudgetReservationCommand, BudgetReservationState, BudgetReservationTransition,
            BudgetResource, BudgetScopeLimit, ExecutionBudgetRepository,
            ExecutionBudgetRepositoryError,
        },
        unit_of_work::UnitOfWork,
    };
    use sea_orm::{
        ActiveModelTrait,
        ActiveValue::{NotSet, Set},
        ConnectionTrait, DatabaseBackend, EntityTrait, Statement,
    };
    use uuid::Uuid;

    use crate::{
        DatabaseHandle, PostgresUnitOfWork,
        entities::{sea_orm_active_enums::StrategyTaskPhase, strategy, strategy_task},
    };

    use super::PostgresExecutionBudgetRepository;

    #[derive(Debug, PartialEq, Eq)]
    struct LedgerCounts {
        reserved: i64,
        consumed: i64,
        released: i64,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct ReservationObservation {
        first_reservations: usize,
        replay_matches_original: bool,
        second_reservations: usize,
        over_limit_rejected: bool,
        changed_request_rejected: bool,
        duplicate_release_is_idempotent: bool,
        released_call_cannot_be_reused: bool,
        consumed_call_cannot_be_released: bool,
        retry_has_distinct_reservation: bool,
        separate_task_reservations: usize,
        expired_call_rejected: bool,
        unmeasured_task_rejected: bool,
        first_task_counter: i64,
        separate_task_counter: i64,
        expired_task_counter: i64,
        unmeasured_task_counter: i64,
        first_task_ledger: LedgerCounts,
    }

    #[backend_test_macros::database_test]
    async fn reservations_are_idempotent_scoped_and_persisted(db: DatabaseHandle) {
        let now = Utc::now().fixed_offset().trunc_subsecs(6);
        let strategy_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let other_task_id = Uuid::new_v4();
        let expired_task_id = Uuid::new_v4();
        let unmeasured_task_id = Uuid::new_v4();
        let repository = PostgresExecutionBudgetRepository::new();
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        insert_strategy(&db, strategy_id).await;
        insert_task(
            &db,
            strategy_id,
            task_id,
            now + chrono::Duration::minutes(2),
        )
        .await;
        insert_task(
            &db,
            strategy_id,
            other_task_id,
            now + chrono::Duration::minutes(2),
        )
        .await;
        insert_task(
            &db,
            strategy_id,
            expired_task_id,
            now - chrono::Duration::seconds(1),
        )
        .await;
        insert_task(
            &db,
            strategy_id,
            unmeasured_task_id,
            now + chrono::Duration::minutes(2),
        )
        .await;
        for (id, deadline) in [
            (task_id, now + chrono::Duration::minutes(2)),
            (other_task_id, now + chrono::Duration::minutes(2)),
            (expired_task_id, now - chrono::Duration::seconds(1)),
            (unmeasured_task_id, now + chrono::Duration::minutes(2)),
        ] {
            let transaction = unit_of_work.begin().await.expect("begin budget setup");
            repository
                .initialize_task(&transaction, id, deadline)
                .await
                .expect("initialize task budget");
            unit_of_work
                .commit(transaction)
                .await
                .expect("commit budget setup");
        }
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE task_execution_state SET usage_complete = false WHERE task_id = $1",
            [unmeasured_task_id.into()],
        ))
        .await
        .expect("mark legacy usage as unmeasured");

        let first = reserve(
            &repository,
            &unit_of_work,
            command(task_id, "call-one", now, 2),
        )
        .await
        .expect("reserve first call");
        let replay = reserve(
            &repository,
            &unit_of_work,
            command(task_id, "call-one", now, 2),
        )
        .await
        .expect("replay first call");
        let second = reserve(
            &repository,
            &unit_of_work,
            command(task_id, "call-two", now, 2),
        )
        .await
        .expect("reserve second call");
        let over_limit = reserve(
            &repository,
            &unit_of_work,
            command(task_id, "call-three-before-release", now, 2),
        )
        .await;
        let mut changed_request = command(task_id, "call-one", now, 2);
        changed_request.request_fingerprint = "different-request-content".to_string();
        let changed_request = reserve(&repository, &unit_of_work, changed_request).await;
        transition(
            &repository,
            &unit_of_work,
            task_id,
            "call-one",
            BudgetReservationState::Released,
            now,
        )
        .await
        .expect("release known-unsent call");
        let duplicate_release = transition(
            &repository,
            &unit_of_work,
            task_id,
            "call-one",
            BudgetReservationState::Released,
            now,
        )
        .await;
        let released_replay = reserve(
            &repository,
            &unit_of_work,
            command(task_id, "call-one", now, 2),
        )
        .await;
        transition(
            &repository,
            &unit_of_work,
            task_id,
            "call-two",
            BudgetReservationState::Consumed,
            now,
        )
        .await
        .expect("mark sent call consumed");
        let consumed_release = transition(
            &repository,
            &unit_of_work,
            task_id,
            "call-two",
            BudgetReservationState::Released,
            now,
        )
        .await;
        let retried_call = reserve(
            &repository,
            &unit_of_work,
            command(task_id, "call-three", now, 2),
        )
        .await
        .expect("reserve actual retry as a new call");
        let separate_task = reserve(
            &repository,
            &unit_of_work,
            command(other_task_id, "call-two", now, 2),
        )
        .await
        .expect("reserve under a separate task budget");
        let expired = reserve(
            &repository,
            &unit_of_work,
            command(expired_task_id, "expired-call", now, 2),
        )
        .await;
        let unmeasured = reserve(
            &repository,
            &unit_of_work,
            command(unmeasured_task_id, "unmeasured-call", now, 2),
        )
        .await;

        let actual = ReservationObservation {
            first_reservations: first.reservations.len(),
            replay_matches_original: replay.reservations == first.reservations,
            second_reservations: second.reservations.len(),
            over_limit_rejected: matches!(
                over_limit,
                Err(ExecutionBudgetRepositoryError::LimitExceeded { .. })
            ),
            changed_request_rejected: matches!(
                changed_request,
                Err(ExecutionBudgetRepositoryError::ReservationConflict)
            ),
            duplicate_release_is_idempotent: duplicate_release.is_ok(),
            released_call_cannot_be_reused: matches!(
                released_replay,
                Err(ExecutionBudgetRepositoryError::ReservationConflict)
            ),
            consumed_call_cannot_be_released: matches!(
                consumed_release,
                Err(ExecutionBudgetRepositoryError::InvalidReservationState)
            ),
            retry_has_distinct_reservation: retried_call.reservations[0].reservation_id
                != first.reservations[0].reservation_id,
            separate_task_reservations: separate_task.reservations.len(),
            expired_call_rejected: matches!(
                expired,
                Err(ExecutionBudgetRepositoryError::DeadlineExpired)
            ),
            unmeasured_task_rejected: matches!(
                unmeasured,
                Err(ExecutionBudgetRepositoryError::UsageNotMeasured)
            ),
            first_task_counter: task_counter(&db, task_id).await,
            separate_task_counter: task_counter(&db, other_task_id).await,
            expired_task_counter: task_counter(&db, expired_task_id).await,
            unmeasured_task_counter: task_counter(&db, unmeasured_task_id).await,
            first_task_ledger: ledger_counts(&db, task_id).await,
        };

        assert_eq!(
            actual,
            ReservationObservation {
                first_reservations: 1,
                replay_matches_original: true,
                second_reservations: 1,
                over_limit_rejected: true,
                changed_request_rejected: true,
                duplicate_release_is_idempotent: true,
                released_call_cannot_be_reused: true,
                consumed_call_cannot_be_released: true,
                retry_has_distinct_reservation: true,
                separate_task_reservations: 1,
                expired_call_rejected: true,
                unmeasured_task_rejected: true,
                first_task_counter: 2,
                separate_task_counter: 1,
                expired_task_counter: 0,
                unmeasured_task_counter: 0,
                first_task_ledger: LedgerCounts {
                    reserved: 1,
                    consumed: 1,
                    released: 1,
                },
            },
        );
    }

    #[tokio::test]
    async fn parallel_reservations_cannot_exceed_the_scope_limit() {
        let pool = crate::test_support::create_test_pool_with_max_connections(3).await;
        let db = DatabaseHandle::from(sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(
            pool.clone(),
        ));
        let now = Utc::now().fixed_offset().trunc_subsecs(6);
        let strategy_id = Uuid::new_v4();
        let task_id = Uuid::new_v4();
        let deadline_at = now + chrono::Duration::minutes(2);
        insert_strategy(&db, strategy_id).await;
        insert_task(&db, strategy_id, task_id, deadline_at).await;
        let repository = PostgresExecutionBudgetRepository::new();
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin budget setup");
        repository
            .initialize_task(&transaction, task_id, deadline_at)
            .await
            .expect("initialize task budget");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit budget setup");

        let (first, second) = tokio::join!(
            reserve(
                &repository,
                &unit_of_work,
                command(task_id, "parallel-call-one", now, 1),
            ),
            reserve(
                &repository,
                &unit_of_work,
                command(task_id, "parallel-call-two", now, 1),
            ),
        );
        let results = [first, second];
        let actual = (
            results.iter().filter(|result| result.is_ok()).count(),
            results
                .iter()
                .filter(|result| {
                    matches!(
                        result,
                        Err(ExecutionBudgetRepositoryError::LimitExceeded { .. })
                    )
                })
                .count(),
            task_counter(&db, task_id).await,
            ledger_counts(&db, task_id).await,
        );
        strategy::Entity::delete_by_id(strategy_id)
            .exec(&db)
            .await
            .expect("clean up concurrent reservation fixture");

        assert_eq!(
            actual,
            (
                1,
                1,
                1,
                LedgerCounts {
                    reserved: 1,
                    consumed: 0,
                    released: 0,
                },
            ),
        );
    }

    async fn insert_strategy(db: &DatabaseHandle, strategy_id: Uuid) {
        strategy::ActiveModel {
            id: Set(strategy_id),
            name: Set("Fictional strategy".to_string()),
            description: Set(None),
            sort_order: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test strategy");
    }

    async fn insert_task(
        db: &DatabaseHandle,
        strategy_id: Uuid,
        task_id: Uuid,
        deadline_at: DateTime<FixedOffset>,
    ) {
        strategy_task::ActiveModel {
            task_id: Set(task_id),
            strategy_id: Set(strategy_id),
            a2a_task_id: Set(None),
            source: Set("review".to_string()),
            prompt: Set("Fictional prompt".to_string()),
            phase: Set(StrategyTaskPhase::Pending),
            error_summary: Set(None),
            result_text: Set(None),
            deadline_at: Set(deadline_at),
            purpose: Set(Some("fictional-purpose".to_string())),
            as_of: Set(Some(deadline_at)),
            context_kind: NotSet,
            context_id: NotSet,
            auto_resumed_at: NotSet,
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(db)
        .await
        .expect("insert test task");
    }

    fn command(
        task_id: Uuid,
        call_id: &str,
        now: DateTime<FixedOffset>,
        limit: u64,
    ) -> BudgetReservationCommand {
        BudgetReservationCommand {
            task_id,
            call_id: call_id.to_string(),
            request_fingerprint: format!("fingerprint-{call_id}"),
            resource: BudgetResource::ModelCall,
            charges: vec![BudgetScopeLimit {
                resource_key: "model_call".to_string(),
                scope_key: "step:00000000-0000-0000-0000-000000000001".to_string(),
                amount: 1,
                limit: Some(limit),
            }],
            now,
        }
    }

    async fn reserve(
        repository: &PostgresExecutionBudgetRepository,
        unit_of_work: &PostgresUnitOfWork,
        command: BudgetReservationCommand,
    ) -> Result<
        core_application::execution_budget::BudgetReservationResult,
        ExecutionBudgetRepositoryError,
    > {
        let transaction = unit_of_work
            .begin()
            .await
            .expect("begin reservation transaction");
        let result = repository.reserve(&transaction, command).await;
        if result.is_ok() {
            unit_of_work
                .commit(transaction)
                .await
                .expect("commit reservation");
        }
        result
    }

    async fn transition(
        repository: &PostgresExecutionBudgetRepository,
        unit_of_work: &PostgresUnitOfWork,
        task_id: Uuid,
        call_id: &str,
        state: BudgetReservationState,
        now: DateTime<FixedOffset>,
    ) -> Result<BudgetReservationTransition, ExecutionBudgetRepositoryError> {
        let transaction = unit_of_work
            .begin()
            .await
            .expect("begin reservation transition");
        let result = repository
            .transition_call(&transaction, task_id, call_id, state, now)
            .await;
        if result.is_ok() {
            unit_of_work
                .commit(transaction)
                .await
                .expect("commit reservation transition");
        }
        result
    }

    async fn task_counter(db: &DatabaseHandle, task_id: Uuid) -> i64 {
        db.query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT model_calls_used FROM task_execution_state WHERE task_id = $1",
            [task_id.into()],
        ))
        .await
        .expect("query execution counter")
        .expect("execution state exists")
        .try_get("", "model_calls_used")
        .expect("read model call counter")
    }

    async fn ledger_counts(db: &DatabaseHandle, task_id: Uuid) -> LedgerCounts {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT COUNT(*) FILTER (WHERE state = 'reserved') AS reserved, COUNT(*) FILTER (WHERE state = 'consumed') AS consumed, COUNT(*) FILTER (WHERE state = 'released') AS released FROM execution_reservation WHERE task_id = $1",
                [task_id.into()],
            ))
            .await
            .expect("query reservation ledger")
            .expect("reservation counts exist");
        LedgerCounts {
            reserved: row.try_get("", "reserved").expect("read reserved count"),
            consumed: row.try_get("", "consumed").expect("read consumed count"),
            released: row.try_get("", "released").expect("read released count"),
        }
    }
}
