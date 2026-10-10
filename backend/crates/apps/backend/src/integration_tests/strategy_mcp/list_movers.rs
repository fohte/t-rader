use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
use core_domain::bar::{Bar, Timeframe};
use gateway_postgres::entities::{
    instruments, note_version, paper_account, paper_order, prediction, stock,
    strategy_task_step_evidence, trade,
};
use gateway_postgres::repositories::bars::upsert_bars;
use rust_decimal::Decimal;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{Value, json};
use uuid::Uuid;

use super::test_server::StrategyServer;
use super::tests_common::{build_server, insert_strategy};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("valid fixture date")
}

fn instant(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).expect("valid fixture timestamp")
}

fn bar(instrument_id: &str, date: NaiveDate, close: i64, volume: i64) -> Bar {
    let timestamp = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("valid fixture time"));
    Bar {
        instrument_id: instrument_id.to_string(),
        timeframe: Timeframe::Daily,
        timestamp,
        open: Decimal::from(close),
        high: Decimal::from(close + 1),
        low: Decimal::from(close - 1),
        close: Decimal::from(close),
        volume,
        adjustment_factor: Decimal::ONE,
    }
}

async fn insert_instrument(db: &impl sea_orm::ConnectionTrait, id: &str) {
    instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(format!("Fictional {id}")),
        market: Set("TSE".to_string()),
        sector: Set(None),
    }
    .insert(db)
    .await
    .expect("insert fictional instrument");
}

async fn insert_stock(db: &impl sea_orm::ConnectionTrait, id: &str) {
    stock::ActiveModel {
        id: Set(id.to_string()),
        name: Set(format!("Fictional {id}")),
        market: Set(None),
        product_category: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert fictional stock");
}

async fn insert_activity_step(db: &impl sea_orm::ConnectionTrait, strategy_id: Uuid) -> Uuid {
    let execution_step_id = Uuid::new_v4();
    crate::testing::insert_test_strategy_task_step(db, strategy_id, execution_step_id).await;
    execution_step_id
}

async fn insert_query_data_evidence(
    db: &impl sea_orm::ConnectionTrait,
    strategy_id: Uuid,
    instrument_id: &str,
    observed_at: DateTime<FixedOffset>,
) {
    let execution_step_id = insert_activity_step(db, strategy_id).await;
    strategy_task_step_evidence::ActiveModel {
        id: Set(Uuid::new_v4()),
        execution_step_id: Set(execution_step_id),
        source: Set("query_data".to_string()),
        source_ref: Set(instrument_id.to_string()),
        observed_at: Set(observed_at),
        published_at: Set(None),
        effective_at: Set(None),
        snapshot: Set(json!({"fixture": true})),
    }
    .insert(db)
    .await
    .expect("insert query data evidence");
}

async fn insert_note_contact(
    db: &gateway_postgres::DatabaseHandle,
    strategy_id: Uuid,
    instrument_id: &str,
    created_at: DateTime<FixedOffset>,
) -> Uuid {
    let execution_step_id = insert_activity_step(db, strategy_id).await;
    let note_id = crate::testing::insert_test_note(
        db,
        "Fictional note",
        &format!("[[stock:{instrument_id}]]"),
    )
    .await;
    let version = note_version::Entity::find()
        .filter(note_version::Column::NoteId.eq(note_id))
        .one(db)
        .await
        .expect("find fictional note version")
        .expect("fictional note version exists");
    note_version::Entity::update_many()
        .col_expr(
            note_version::Column::ExecutionId,
            Expr::value(Some(execution_step_id.to_string())),
        )
        .col_expr(note_version::Column::CreatedAt, Expr::value(created_at))
        .filter(note_version::Column::Id.eq(version.id))
        .exec(db)
        .await
        .expect("set note contact time and execution step");
    version.id
}

async fn insert_trade_contact(
    db: &impl sea_orm::ConnectionTrait,
    strategy_id: Uuid,
    instrument_id: &str,
    date: NaiveDate,
) {
    trade::ActiveModel {
        id: Set(Uuid::new_v4()),
        strategy_id: Set(strategy_id),
        symbol: Set(instrument_id.to_string()),
        side: Set("buy".to_string()),
        qty: Set(Decimal::ONE),
        price: Set(Decimal::from(100)),
        fee: Set(Decimal::ZERO),
        date: Set(date),
        source: Set("manual".to_string()),
        note: Set(None),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert fictional trade");
}

async fn insert_paper_trade_contact(
    db: &gateway_postgres::DatabaseHandle,
    strategy_id: Uuid,
    instrument_id: &str,
    note_version_id: Uuid,
    ordered_at: DateTime<FixedOffset>,
) {
    let purpose = format!("fixture-purpose-{}", Uuid::new_v4());
    crate::testing::agent_config::create(db, purpose.clone())
        .await
        .expect("create fixture agent config");
    let account_id = Uuid::new_v4();
    paper_account::ActiveModel {
        id: Set(account_id),
        name: Set(format!("Fictional account {}", Uuid::new_v4())),
        strategy_id: Set(strategy_id),
        purpose: Set(purpose),
        initial_cash_jpy: Set(Decimal::from(100_000)),
        benchmark_stock_id: Set(None),
        started_on: Set(date(2025, 1, 1)),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert fictional paper account");
    paper_order::ActiveModel {
        id: Set(Uuid::new_v4()),
        account_id: Set(account_id),
        stock_id: Set(instrument_id.to_string()),
        side: Set("buy".to_string()),
        qty: Set(100),
        note_version_id: Set(note_version_id),
        ordered_at: Set(ordered_at),
    }
    .insert(db)
    .await
    .expect("insert fictional paper order");
}

async fn insert_prediction_contact(
    db: &impl sea_orm::ConnectionTrait,
    strategy_id: Uuid,
    instrument_id: &str,
    created_at: DateTime<FixedOffset>,
) {
    prediction::ActiveModel {
        prediction_id: Set(Uuid::new_v4()),
        strategy_id: Set(strategy_id),
        note_id: Set(None),
        target_stock_id: Set(instrument_id.to_string()),
        benchmark_stock_id: Set("fictional-benchmark".to_string()),
        direction: Set("outperform".to_string()),
        probability: Set(Decimal::new(55, 2)),
        base_date: Set(date(2025, 2, 3)),
        due_date: Set(date(2025, 2, 10)),
        created_at: Set(created_at),
    }
    .insert(db)
    .await
    .expect("insert fictional prediction");
}

async fn seed_movers(db: &gateway_postgres::DatabaseHandle, strategy_id: Uuid) -> Uuid {
    let ids = [
        "fictional-query",
        "fictional-note",
        "fictional-trade",
        "fictional-paper",
        "fictional-prediction",
        "fictional-unseen",
        "fictional-illiquid",
    ];
    for id in ids {
        insert_instrument(db, id).await;
        insert_stock(db, id).await;
    }
    insert_stock(db, "fictional-benchmark").await;

    upsert_bars(
        db,
        vec![
            bar("fictional-query", date(2025, 1, 31), 100, 100),
            bar("fictional-query", date(2025, 2, 3), 120, 100),
            bar("fictional-query", date(2025, 2, 4), 150, 100),
            bar("fictional-note", date(2025, 1, 31), 100, 100),
            bar("fictional-note", date(2025, 2, 3), 80, 100),
            bar("fictional-note", date(2025, 2, 4), 70, 100),
            bar("fictional-trade", date(2025, 1, 31), 100, 100),
            bar("fictional-trade", date(2025, 2, 3), 130, 100),
            bar("fictional-trade", date(2025, 2, 4), 130, 100),
            bar("fictional-paper", date(2025, 1, 31), 100, 100),
            bar("fictional-paper", date(2025, 2, 3), 120, 100),
            bar("fictional-paper", date(2025, 2, 4), 90, 100),
            bar("fictional-prediction", date(2025, 1, 31), 100, 100),
            bar("fictional-prediction", date(2025, 2, 3), 105, 100),
            bar("fictional-prediction", date(2025, 2, 4), 105, 100),
            bar("fictional-unseen", date(2025, 1, 31), 100, 100),
            bar("fictional-unseen", date(2025, 2, 3), 105, 100),
            bar("fictional-unseen", date(2025, 2, 4), 110, 100),
            bar("fictional-illiquid", date(2025, 1, 31), 100, 1),
            bar("fictional-illiquid", date(2025, 2, 3), 150, 1),
            bar("fictional-illiquid", date(2025, 2, 4), 200, 1),
        ],
    )
    .await
    .expect("insert fictional daily bars");

    insert_query_data_evidence(
        db,
        strategy_id,
        "fictional-query",
        instant("2025-02-04T12:30:00Z"),
    )
    .await;
    let note_version_id = insert_note_contact(
        db,
        strategy_id,
        "fictional-note",
        instant("2025-02-03T09:15:00Z"),
    )
    .await;
    insert_trade_contact(db, strategy_id, "fictional-trade", date(2025, 2, 3)).await;
    insert_paper_trade_contact(
        db,
        strategy_id,
        "fictional-paper",
        note_version_id,
        instant("2025-02-04T01:02:00Z"),
    )
    .await;
    insert_prediction_contact(
        db,
        strategy_id,
        "fictional-prediction",
        instant("2025-02-04T06:00:00Z"),
    )
    .await;

    strategy_id
}

fn expected_mover(
    instrument_id: &str,
    change_rate: f64,
    avg_turnover: f64,
    first_seen_at: Option<DateTime<FixedOffset>>,
    seen_via: Option<&str>,
) -> Value {
    json!({
        "instrument_id": instrument_id,
        "name": format!("Fictional {instrument_id}"),
        "change_rate": change_rate,
        "avg_turnover": avg_turnover,
        "first_seen_at": first_seen_at,
        "seen_via": seen_via,
    })
}

#[backend_test_macros::database_test]
async fn list_movers_ranks_returns_filters_turnover_and_scopes_first_contact(
    db: gateway_postgres::DatabaseHandle,
) {
    let strategy_id = insert_strategy(&db, "Fictional strategy").await;
    let other_strategy_id = insert_strategy(&db, "Another fictional strategy").await;
    seed_movers(&db, strategy_id).await;
    insert_query_data_evidence(
        &db,
        other_strategy_id,
        "fictional-unseen",
        instant("2025-02-03T10:00:00Z"),
    )
    .await;

    let server = build_server(db);
    let all = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-03",
                "to": "2025-02-09",
                "direction": "abs",
                "limit": 7,
            }),
            None,
            None,
        )
        .await
        .expect("list all movers")
        .as_json()
        .clone();
    let up = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-03",
                "to": "2025-02-04",
                "direction": "up",
                "min_avg_turnover": 10_000,
                "limit": 2,
            }),
            None,
            None,
        )
        .await
        .expect("list positive movers")
        .as_json()
        .clone();
    let down = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-03",
                "to": "2025-02-04",
                "direction": "down",
                "limit": 2,
            }),
            None,
            None,
        )
        .await
        .expect("list negative movers")
        .as_json()
        .clone();
    let one_day = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-04",
                "to": "2025-02-04",
                "direction": "up",
                "min_avg_turnover": 1_000,
                "limit": 1,
            }),
            None,
            None,
        )
        .await
        .expect("list one-day movers")
        .as_json()
        .clone();

    assert_eq!(
        (all, up, down, one_day),
        (
            json!({
                "movers": [
                    expected_mover(
                        "fictional-illiquid",
                        1.0,
                        175.0,
                        None,
                        None,
                    ),
                    expected_mover(
                        "fictional-query",
                        0.5,
                        13_500.0,
                        Some(instant("2025-02-04T12:30:00Z")),
                        Some("query_data"),
                    ),
                    expected_mover(
                        "fictional-note",
                        -0.3,
                        7_500.0,
                        Some(instant("2025-02-03T09:15:00Z")),
                        Some("note"),
                    ),
                    expected_mover(
                        "fictional-trade",
                        0.3,
                        13_000.0,
                        Some(instant("2025-02-03T00:00:00Z")),
                        Some("trade"),
                    ),
                    expected_mover(
                        "fictional-paper",
                        -0.1,
                        10_500.0,
                        Some(instant("2025-02-04T01:02:00Z")),
                        Some("paper_trade"),
                    ),
                    expected_mover(
                        "fictional-unseen",
                        0.1,
                        10_750.0,
                        None,
                        None,
                    ),
                    expected_mover(
                        "fictional-prediction",
                        0.05,
                        10_500.0,
                        Some(instant("2025-02-04T06:00:00Z")),
                        Some("prediction"),
                    ),
                ],
            }),
            json!({
                "movers": [
                    expected_mover(
                        "fictional-query",
                        0.5,
                        13_500.0,
                        Some(instant("2025-02-04T12:30:00Z")),
                        Some("query_data"),
                    ),
                    expected_mover(
                        "fictional-trade",
                        0.3,
                        13_000.0,
                        Some(instant("2025-02-03T00:00:00Z")),
                        Some("trade"),
                    ),
                ],
            }),
            json!({
                "movers": [
                    expected_mover(
                        "fictional-note",
                        -0.3,
                        7_500.0,
                        Some(instant("2025-02-03T09:15:00Z")),
                        Some("note"),
                    ),
                    expected_mover(
                        "fictional-paper",
                        -0.1,
                        10_500.0,
                        Some(instant("2025-02-04T01:02:00Z")),
                        Some("paper_trade"),
                    ),
                ],
            }),
            json!({
                "movers": [
                    expected_mover(
                        "fictional-query",
                        0.25,
                        15_000.0,
                        Some(instant("2025-02-04T12:30:00Z")),
                        Some("query_data"),
                    ),
                ],
            }),
        ),
    );
}

#[backend_test_macros::database_test]
async fn list_movers_rejects_invalid_date_range_and_limits(db: gateway_postgres::DatabaseHandle) {
    let strategy_id = insert_strategy(&db, "Fictional strategy").await;
    let server: StrategyServer = build_server(db);
    let reversed_range = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-04",
                "to": "2025-02-03",
                "direction": "abs",
            }),
            None,
            None,
        )
        .await
        .map(|_| ());
    let zero_limit = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-03",
                "to": "2025-02-04",
                "direction": "abs",
                "limit": 0,
            }),
            None,
            None,
        )
        .await
        .map(|_| ());
    let oversized_limit = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-03",
                "to": "2025-02-04",
                "direction": "abs",
                "limit": 101,
            }),
            None,
            None,
        )
        .await
        .map(|_| ());
    let negative_turnover = server
        .invoke::<_, Value>(
            "list_movers",
            strategy_id,
            json!({
                "from": "2025-02-03",
                "to": "2025-02-04",
                "direction": "abs",
                "min_avg_turnover": -1,
            }),
            None,
            None,
        )
        .await
        .map(|_| ());

    assert_eq!(
        (
            reversed_range,
            zero_limit,
            oversized_limit,
            negative_turnover,
        ),
        (
            Err(rmcp::ErrorData::invalid_params(
                "from must be on or before to",
                None,
            )),
            Err(rmcp::ErrorData::invalid_params(
                "limit must be between 1 and 100",
                None,
            )),
            Err(rmcp::ErrorData::invalid_params(
                "limit must be between 1 and 100",
                None,
            )),
            Err(rmcp::ErrorData::invalid_params(
                "min_avg_turnover must be a non-negative number",
                None,
            )),
        ),
    );
}
