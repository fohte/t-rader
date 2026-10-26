use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use core_application::tavily_credit_budget::{
    BootstrapCommand, FinishFailureCommand, RecordCacheHitCommand, ReservationClaim,
    ReservationCommand, ReservationFailureState, ReservationOutcome, TavilyCreditBudgetConfig,
    TavilyCreditBudgetDenial, TavilyCreditBudgetOperation, TavilyCreditBudgetReason,
    TavilyCreditBudgetRepository, TavilyCreditBudgetScope,
};
use core_application::unit_of_work::UnitOfWork;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, SqlxPostgresConnector};
use serde_json::json;

use super::{
    PostgresTavilyCreditBudgetRepository,
    support::{load_reservation, load_state},
};
use crate::transaction::transaction_ref;
use crate::{DatabaseHandle, PostgresUnitOfWork};

fn utc(day: NaiveDate, hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.from_utc_datetime(&day.and_hms_opt(hour, minute, 0).expect("valid test time"))
}

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid test date")
}

async fn reserve(
    repository: &PostgresTavilyCreditBudgetRepository,
    unit_of_work: &PostgresUnitOfWork,
    command: ReservationCommand,
) -> ReservationOutcome {
    let transaction = unit_of_work.begin().await.expect("begin reservation");
    let outcome = repository
        .reserve(&transaction, command)
        .await
        .expect("reserve account credits");
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit reservation");
    outcome
}

async fn claim(
    repository: &PostgresTavilyCreditBudgetRepository,
    unit_of_work: &PostgresUnitOfWork,
    call_id: &str,
    fingerprint: &str,
    now: DateTime<Utc>,
) {
    let transaction = unit_of_work.begin().await.expect("begin claim");
    let result = repository
        .claim(&transaction, call_id, fingerprint, now)
        .await
        .expect("claim reservation");
    assert_eq!(result, ReservationClaim::Claimed);
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit claim");
}

#[backend_test_macros::database_test]
async fn unknown_reservation_remains_in_budget_after_month_reset(db: DatabaseHandle) {
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let repository = PostgresTavilyCreditBudgetRepository::new();
    let config = TavilyCreditBudgetConfig::free_tier("example-account".into(), true)
        .expect("valid synthetic config");
    let jan_four = day(2026, 1, 4);
    let jan_five = day(2026, 1, 5);
    let feb_one = day(2026, 2, 1);
    let bootstrap = unit_of_work.begin().await.expect("begin bootstrap");
    repository
        .initialize(
            &bootstrap,
            BootstrapCommand {
                config: config.clone(),
                monthly_provider_usage: Decimal::ZERO,
                month_start: day(2026, 1, 1),
                day_start: jan_four,
                daily_blocked_until: utc(jan_five, 0, 0),
                now: utc(jan_four, 23, 59),
            },
        )
        .await
        .expect("initialize account budget");
    unit_of_work
        .commit(bootstrap)
        .await
        .expect("commit bootstrap");

    let first = ReservationCommand {
        config: config.clone(),
        call_id: "unknown-call".into(),
        request_fingerprint: "a".repeat(64),
        operation: TavilyCreditBudgetOperation::Search,
        monthly_provider_usage: Decimal::new(24, 0),
        month_start: day(2026, 1, 1),
        day_start: jan_five,
        now: utc(jan_five, 0, 0),
    };
    assert_eq!(
        reserve(&repository, &unit_of_work, first.clone()).await,
        ReservationOutcome::Reserved {
            daily_used: Decimal::new(25, 0),
            monthly_used: Decimal::new(25, 0),
        },
    );
    claim(
        &repository,
        &unit_of_work,
        &first.call_id,
        &first.request_fingerprint,
        first.now,
    )
    .await;
    let failed = unit_of_work.begin().await.expect("begin failure state");
    repository
        .finish_failure(
            &failed,
            FinishFailureCommand {
                call_id: first.call_id.clone(),
                request_fingerprint: first.request_fingerprint.clone(),
                state: ReservationFailureState::Unknown,
                outcome: json!({"kind": "unknown"}),
                now: first.now,
            },
        )
        .await
        .expect("retain unknown reservation");
    unit_of_work
        .commit(failed)
        .await
        .expect("commit failure state");

    assert_eq!(
        reserve(
            &repository,
            &unit_of_work,
            ReservationCommand {
                config: config.clone(),
                call_id: "daily-limit-call".into(),
                request_fingerprint: "b".repeat(64),
                operation: TavilyCreditBudgetOperation::Search,
                monthly_provider_usage: Decimal::new(24, 0),
                month_start: day(2026, 1, 1),
                day_start: jan_five,
                now: utc(jan_five, 0, 1),
            },
        )
        .await,
        ReservationOutcome::Denied {
            denial: TavilyCreditBudgetDenial::new(
                TavilyCreditBudgetReason::DailyLimitExceeded,
                TavilyCreditBudgetScope::Daily,
                Some(utc(day(2026, 1, 6), 0, 0)),
            ),
            daily_used: Decimal::new(25, 0),
            monthly_used: Decimal::new(25, 0),
        },
    );

    assert_eq!(
        reserve(
            &repository,
            &unit_of_work,
            ReservationCommand {
                config: config.clone(),
                call_id: "after-month-reset".into(),
                request_fingerprint: "c".repeat(64),
                operation: TavilyCreditBudgetOperation::Search,
                monthly_provider_usage: Decimal::ZERO,
                month_start: day(2026, 2, 1),
                day_start: feb_one,
                now: utc(feb_one, 0, 0),
            },
        )
        .await,
        ReservationOutcome::Reserved {
            daily_used: Decimal::new(2, 0),
            monthly_used: Decimal::new(2, 0),
        },
    );

    let after_reset = ReservationCommand {
        config,
        call_id: "after-month-reset".into(),
        request_fingerprint: "c".repeat(64),
        operation: TavilyCreditBudgetOperation::Search,
        monthly_provider_usage: Decimal::ZERO,
        month_start: day(2026, 2, 1),
        day_start: feb_one,
        now: utc(feb_one, 0, 0),
    };
    claim(
        &repository,
        &unit_of_work,
        &after_reset.call_id,
        &after_reset.request_fingerprint,
        after_reset.now,
    )
    .await;
    let settled = unit_of_work.begin().await.expect("begin settlement");
    repository
        .settle(
            &settled,
            &after_reset.call_id,
            &after_reset.request_fingerprint,
            Decimal::ONE,
            json!({"kind": "settled"}),
            utc(feb_one, 0, 1),
        )
        .await
        .expect("settle current period request");
    unit_of_work
        .commit(settled)
        .await
        .expect("commit settlement");

    assert_eq!(
        reserve(
            &repository,
            &unit_of_work,
            ReservationCommand {
                config: after_reset.config,
                call_id: "external-usage-call".into(),
                request_fingerprint: "d".repeat(64),
                operation: TavilyCreditBudgetOperation::Search,
                monthly_provider_usage: Decimal::new(898, 0),
                month_start: day(2026, 2, 1),
                day_start: feb_one,
                now: utc(feb_one, 0, 2),
            },
        )
        .await,
        ReservationOutcome::Denied {
            denial: TavilyCreditBudgetDenial::new(
                TavilyCreditBudgetReason::DailyLimitExceeded,
                TavilyCreditBudgetScope::Daily,
                Some(utc(day(2026, 2, 2), 0, 0)),
            ),
            daily_used: Decimal::new(899, 0),
            monthly_used: Decimal::new(899, 0),
        },
    );

    let transaction = unit_of_work.begin().await.expect("begin state inspection");
    let state = load_state(
        transaction_ref(&transaction).expect("postgres transaction"),
        false,
    )
    .await
    .expect("load account state")
    .expect("account state exists");
    assert_eq!(
        (
            state.daily_used,
            state.daily_carry,
            state.monthly_used,
            state.monthly_carry,
        ),
        (
            Decimal::new(899, 0),
            Decimal::ONE,
            Decimal::new(899, 0),
            Decimal::ONE,
        ),
    );
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit state inspection");
}

#[backend_test_macros::database_test]
async fn provider_usage_includes_dispatching_reservations_without_double_counting(
    db: DatabaseHandle,
) {
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let repository = PostgresTavilyCreditBudgetRepository::new();
    let config = TavilyCreditBudgetConfig::free_tier("example-account".into(), true)
        .expect("valid synthetic config");
    let current_day = day(2026, 3, 2);
    let bootstrap = unit_of_work.begin().await.expect("begin bootstrap");
    repository
        .initialize(
            &bootstrap,
            BootstrapCommand {
                config: config.clone(),
                monthly_provider_usage: Decimal::ZERO,
                month_start: day(2026, 3, 1),
                day_start: current_day,
                daily_blocked_until: utc(current_day, 0, 0),
                now: utc(current_day, 0, 0),
            },
        )
        .await
        .expect("initialize account budget");
    unit_of_work
        .commit(bootstrap)
        .await
        .expect("commit bootstrap");

    let first = ReservationCommand {
        config: config.clone(),
        call_id: "dispatching-call".into(),
        request_fingerprint: "a".repeat(64),
        operation: TavilyCreditBudgetOperation::Search,
        monthly_provider_usage: Decimal::ZERO,
        month_start: day(2026, 3, 1),
        day_start: current_day,
        now: utc(current_day, 0, 1),
    };
    assert_eq!(
        reserve(&repository, &unit_of_work, first.clone()).await,
        ReservationOutcome::Reserved {
            daily_used: Decimal::ONE,
            monthly_used: Decimal::ONE,
        },
    );
    claim(
        &repository,
        &unit_of_work,
        &first.call_id,
        &first.request_fingerprint,
        first.now,
    )
    .await;

    assert_eq!(
        reserve(
            &repository,
            &unit_of_work,
            ReservationCommand {
                config,
                call_id: "next-call".into(),
                request_fingerprint: "b".repeat(64),
                operation: TavilyCreditBudgetOperation::Search,
                monthly_provider_usage: Decimal::ONE,
                month_start: day(2026, 3, 1),
                day_start: current_day,
                now: utc(current_day, 0, 2),
            },
        )
        .await,
        ReservationOutcome::Reserved {
            daily_used: Decimal::new(2, 0),
            monthly_used: Decimal::new(2, 0),
        },
    );
}

#[backend_test_macros::database_test]
async fn concurrent_reservations_are_serialized_by_postgres(_db: DatabaseHandle) {
    let pool = crate::test_support::create_test_pool_with_max_connections(3).await;
    let database = DatabaseHandle::from(SqlxPostgresConnector::from_sqlx_postgres_pool(pool));
    database
        .execute_unprepared("DELETE FROM tavily_credit_reservation")
        .await
        .expect("clear reservations from previous run");
    database
        .execute_unprepared("DELETE FROM tavily_credit_budget_state")
        .await
        .expect("clear budget state from previous run");

    let unit_of_work = PostgresUnitOfWork::new(database.clone());
    let repository = PostgresTavilyCreditBudgetRepository::new();
    let config = TavilyCreditBudgetConfig::free_tier("example-account".into(), true)
        .expect("valid synthetic config");
    let jan_four = day(2026, 1, 4);
    let jan_five = day(2026, 1, 5);
    let bootstrap = unit_of_work.begin().await.expect("begin bootstrap");
    repository
        .initialize(
            &bootstrap,
            BootstrapCommand {
                config: config.clone(),
                monthly_provider_usage: Decimal::ZERO,
                month_start: day(2026, 1, 1),
                day_start: jan_four,
                daily_blocked_until: utc(jan_five, 0, 0),
                now: utc(jan_four, 23, 59),
            },
        )
        .await
        .expect("initialize account budget");
    unit_of_work
        .commit(bootstrap)
        .await
        .expect("commit bootstrap");

    let command = |call_id: &str, fingerprint: char| ReservationCommand {
        config: config.clone(),
        call_id: call_id.into(),
        request_fingerprint: fingerprint.to_string().repeat(64),
        operation: TavilyCreditBudgetOperation::Search,
        monthly_provider_usage: Decimal::new(24, 0),
        month_start: day(2026, 1, 1),
        day_start: jan_five,
        now: utc(jan_five, 0, 0),
    };
    let (first, second) = tokio::join!(
        reserve(&repository, &unit_of_work, command("parallel-one", 'a')),
        reserve(&repository, &unit_of_work, command("parallel-two", 'b')),
    );
    let mut outcomes = vec![first, second];
    outcomes.sort_by_key(|outcome| match outcome {
        ReservationOutcome::Reserved { .. } => 0,
        ReservationOutcome::Denied { .. } => 1,
        _ => 2,
    });
    assert_eq!(
        outcomes,
        vec![
            ReservationOutcome::Reserved {
                daily_used: Decimal::new(25, 0),
                monthly_used: Decimal::new(25, 0),
            },
            ReservationOutcome::Denied {
                denial: TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    Some(utc(day(2026, 1, 6), 0, 0)),
                ),
                daily_used: Decimal::new(25, 0),
                monthly_used: Decimal::new(25, 0),
            },
        ],
    );
    database
        .execute_unprepared("DELETE FROM tavily_credit_reservation")
        .await
        .expect("clear test reservations");
    database
        .execute_unprepared("DELETE FROM tavily_credit_budget_state")
        .await
        .expect("clear test budget state");
}

#[backend_test_macros::database_test]
async fn prepared_reservation_moves_to_the_new_period_once(db: DatabaseHandle) {
    db.execute_unprepared("DELETE FROM tavily_credit_reservation")
        .await
        .expect("clear reservations from previous run");
    db.execute_unprepared("DELETE FROM tavily_credit_budget_state")
        .await
        .expect("clear budget state from previous run");
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let repository = PostgresTavilyCreditBudgetRepository::new();
    let config = TavilyCreditBudgetConfig::free_tier("example-account".into(), true)
        .expect("valid synthetic config");
    let jan_four = day(2026, 1, 4);
    let jan_five = day(2026, 1, 5);
    let feb_one = day(2026, 2, 1);
    let bootstrap = unit_of_work.begin().await.expect("begin bootstrap");
    repository
        .initialize(
            &bootstrap,
            BootstrapCommand {
                config: config.clone(),
                monthly_provider_usage: Decimal::ZERO,
                month_start: day(2026, 1, 1),
                day_start: jan_four,
                daily_blocked_until: utc(jan_five, 0, 0),
                now: utc(jan_four, 23, 59),
            },
        )
        .await
        .expect("initialize account budget");
    unit_of_work
        .commit(bootstrap)
        .await
        .expect("commit bootstrap");

    let prepared = ReservationCommand {
        config,
        call_id: "prepared-call".into(),
        request_fingerprint: "a".repeat(64),
        operation: TavilyCreditBudgetOperation::Search,
        monthly_provider_usage: Decimal::ZERO,
        month_start: day(2026, 1, 1),
        day_start: jan_five,
        now: utc(jan_five, 0, 0),
    };
    let first = reserve(&repository, &unit_of_work, prepared.clone()).await;
    let resumed_command = ReservationCommand {
        month_start: day(2026, 2, 1),
        day_start: feb_one,
        now: utc(feb_one, 0, 0),
        ..prepared.clone()
    };
    let resumed = reserve(&repository, &unit_of_work, resumed_command.clone()).await;
    claim(
        &repository,
        &unit_of_work,
        &resumed_command.call_id,
        &resumed_command.request_fingerprint,
        resumed_command.now,
    )
    .await;
    let settlement = unit_of_work.begin().await.expect("begin settlement");
    repository
        .settle(
            &settlement,
            &resumed_command.call_id,
            &resumed_command.request_fingerprint,
            Decimal::ONE,
            json!({"kind": "settled"}),
            utc(feb_one, 0, 1),
        )
        .await
        .expect("settle resumed reservation");
    unit_of_work
        .commit(settlement)
        .await
        .expect("commit settlement");

    let transaction = unit_of_work.begin().await.expect("begin state inspection");
    let state = load_state(
        transaction_ref(&transaction).expect("postgres transaction"),
        false,
    )
    .await
    .expect("load account state")
    .expect("account state exists");
    assert_eq!(
        (
            first,
            resumed,
            state.daily_used,
            state.daily_carry,
            state.monthly_used,
            state.monthly_carry,
        ),
        (
            ReservationOutcome::Reserved {
                daily_used: Decimal::ONE,
                monthly_used: Decimal::ONE,
            },
            ReservationOutcome::ReusedPrepared {
                daily_used: Decimal::ONE,
                monthly_used: Decimal::ONE,
            },
            Decimal::ONE,
            Decimal::ZERO,
            Decimal::ONE,
            Decimal::ZERO,
        ),
    );
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit state inspection");
}

#[backend_test_macros::database_test]
async fn cache_hit_settles_a_prepared_call_without_consuming_credits(db: DatabaseHandle) {
    db.execute_unprepared("DELETE FROM tavily_credit_reservation")
        .await
        .expect("clear reservations from previous run");
    db.execute_unprepared("DELETE FROM tavily_credit_budget_state")
        .await
        .expect("clear budget state from previous run");
    let unit_of_work = PostgresUnitOfWork::new(db.clone());
    let repository = PostgresTavilyCreditBudgetRepository::new();
    let config = TavilyCreditBudgetConfig::new(
        "synthetic-account".into(),
        Decimal::ONE,
        Decimal::new(2, 0),
        true,
    )
    .expect("valid synthetic config");
    let current_day = day(2026, 3, 2);
    let bootstrap = unit_of_work.begin().await.expect("begin bootstrap");
    repository
        .initialize(
            &bootstrap,
            BootstrapCommand {
                config: config.clone(),
                monthly_provider_usage: Decimal::ZERO,
                month_start: day(2026, 3, 1),
                day_start: current_day,
                daily_blocked_until: utc(current_day, 0, 0),
                now: utc(current_day, 0, 0),
            },
        )
        .await
        .expect("initialize account budget");
    unit_of_work
        .commit(bootstrap)
        .await
        .expect("commit bootstrap");

    let prepared = ReservationCommand {
        config: config.clone(),
        call_id: "cached-call".into(),
        request_fingerprint: "a".repeat(64),
        operation: TavilyCreditBudgetOperation::Search,
        monthly_provider_usage: Decimal::ZERO,
        month_start: day(2026, 3, 1),
        day_start: current_day,
        now: utc(current_day, 0, 1),
    };
    assert_eq!(
        reserve(&repository, &unit_of_work, prepared.clone()).await,
        ReservationOutcome::Reserved {
            daily_used: Decimal::ONE,
            monthly_used: Decimal::ONE,
        },
    );

    let outcome = json!({
        "kind": "search_results_with_fetched_at",
        "value": {
            "results": [],
            "fetched_at": "2026-03-02T00:02:00Z",
        },
    });
    let transaction = unit_of_work.begin().await.expect("begin cache hit");
    assert_eq!(
        repository
            .record_cache_hit(
                &transaction,
                RecordCacheHitCommand {
                    call_id: prepared.call_id.clone(),
                    request_fingerprint: prepared.request_fingerprint.clone(),
                    operation: prepared.operation,
                    outcome: outcome.clone(),
                    now: utc(current_day, 0, 2),
                },
            )
            .await
            .expect("record cache hit"),
        ReservationClaim::Replay(outcome.clone()),
    );
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit cache hit");

    let transaction = unit_of_work.begin().await.expect("begin state inspection");
    let database_transaction = transaction_ref(&transaction).expect("postgres transaction");
    let state = load_state(database_transaction, false)
        .await
        .expect("load account state")
        .expect("account state exists");
    let reservation = load_reservation(database_transaction, &prepared.call_id, false)
        .await
        .expect("load cache hit reservation")
        .expect("cache hit reservation exists");
    assert_eq!(
        (
            state.daily_used,
            state.monthly_used,
            reservation.state,
            reservation.reserved_credits,
            reservation.settled_credits,
            reservation.outcome_json,
        ),
        (
            Decimal::ZERO,
            Decimal::ZERO,
            "settled".to_owned(),
            Decimal::ZERO,
            Some(Decimal::ZERO),
            Some(outcome),
        ),
    );
    unit_of_work
        .commit(transaction)
        .await
        .expect("commit state inspection");

    assert_eq!(
        reserve(
            &repository,
            &unit_of_work,
            ReservationCommand {
                call_id: "available-after-cache-hit".into(),
                request_fingerprint: "b".repeat(64),
                now: utc(current_day, 0, 3),
                ..prepared
            },
        )
        .await,
        ReservationOutcome::Reserved {
            daily_used: Decimal::ONE,
            monthly_used: Decimal::ONE,
        },
    );
}
