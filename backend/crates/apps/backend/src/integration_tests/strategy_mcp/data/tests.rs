use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rstest::rstest;
use rust_decimal::Decimal;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

use super::super::MAX_QUERY_DATA_INSTRUMENTS;
use super::super::StrategyServer;
use super::super::dto::{BarDto, InstrumentBarsDto, QueryDataParams, QueryDataResult};
use super::super::tests_common::{insert_strategy, mock_db_with_strategy};
use core_domain::bar::{Bar, Timeframe};
use gateway_postgres::entities::{instruments, minute_bars, strategy_task_step_evidence};
use gateway_postgres::repositories::bars::upsert_bars;

async fn insert_test_instrument(db: &impl sea_orm::ConnectionTrait, id: &str) {
    instruments::Entity::insert(instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(format!("Test {id}")),
        market: Set("TSE".to_string()),
        sector: Set(None),
    })
    .on_conflict(
        OnConflict::column(instruments::Column::Id)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await
    .expect("failed to insert test instrument");
}

async fn insert_test_instrument_with_market(
    db: &impl sea_orm::ConnectionTrait,
    id: &str,
    market: &str,
) {
    instruments::Entity::insert(instruments::ActiveModel {
        id: Set(id.to_string()),
        name: Set(format!("Test {id}")),
        market: Set(market.to_string()),
        sector: Set(None),
    })
    .on_conflict(
        OnConflict::column(instruments::Column::Id)
            .do_nothing()
            .to_owned(),
    )
    .exec_without_returning(db)
    .await
    .expect("failed to insert test instrument");
}

struct TestMinuteBar {
    hour: u32,
    minute: u32,
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: i64,
}

fn make_test_minute_bar(instrument_id: &str, bar: TestMinuteBar) -> minute_bars::ActiveModel {
    let timestamp = NaiveDate::from_ymd_opt(2034, 1, 2)
        .and_then(|date| date.and_hms_opt(bar.hour, bar.minute, 0))
        .map(|datetime| Utc.from_utc_datetime(&datetime).fixed_offset())
        .expect("valid minute bar timestamp");
    minute_bars::ActiveModel {
        instrument_id: Set(instrument_id.to_string()),
        timestamp: Set(timestamp),
        open: Set(Decimal::new(bar.open, 0)),
        high: Set(Decimal::new(bar.high, 0)),
        low: Set(Decimal::new(bar.low, 0)),
        close: Set(Decimal::new(bar.close, 0)),
        volume: Set(bar.volume),
    }
}

async fn insert_test_minute_bars(db: &impl sea_orm::ConnectionTrait, id: &str, base: i64) {
    minute_bars::Entity::insert_many([
        make_test_minute_bar(
            id,
            TestMinuteBar {
                hour: 14,
                minute: 30,
                open: base,
                high: base + 5,
                low: base - 1,
                close: base + 1,
                volume: 10,
            },
        ),
        make_test_minute_bar(
            id,
            TestMinuteBar {
                hour: 14,
                minute: 31,
                open: base + 1,
                high: base + 4,
                low: base,
                close: base + 2,
                volume: 20,
            },
        ),
    ])
    .exec_without_returning(db)
    .await
    .expect("failed to insert minute bars");
}

fn make_test_bar(instrument_id: &str, date: NaiveDate, close: i64) -> Bar {
    let timestamp = date
        .and_hms_opt(0, 0, 0)
        .map(|dt| Utc.from_utc_datetime(&dt))
        .expect("invalid date");
    Bar {
        instrument_id: instrument_id.to_string(),
        timeframe: Timeframe::Daily,
        timestamp,
        open: Decimal::new(close, 0),
        high: Decimal::new(close + 10, 0),
        low: Decimal::new(close - 10, 0),
        close: Decimal::new(close, 0),
        volume: 1_000,
    }
}

/// `make_test_bar` と同じ OHLCV 規則で期待値の [`BarDto`] を作る
fn bar_dto(date: NaiveDate, close: i64) -> BarDto {
    let timestamp = date
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc().fixed_offset())
        .expect("invalid date");
    BarDto {
        timestamp,
        open: close as f64,
        high: (close + 10) as f64,
        low: (close - 10) as f64,
        close: close as f64,
        volume: 1_000,
    }
}

fn minute_bar_dto(
    hour: u32,
    minute: u32,
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: i64,
) -> BarDto {
    let timestamp = NaiveDate::from_ymd_opt(2034, 1, 2)
        .and_then(|date| date.and_hms_opt(hour, minute, 0))
        .map(|datetime| datetime.and_utc().fixed_offset())
        .expect("valid minute bar timestamp");
    BarDto {
        timestamp,
        open: open as f64,
        high: high as f64,
        low: low as f64,
        close: close as f64,
        volume,
    }
}

async fn setup_server_with_bars(
    db: gateway_postgres::DatabaseHandle,
) -> (gateway_postgres::DatabaseHandle, StrategyServer, Uuid) {
    let strategy_id = insert_strategy(&db, "x").await;

    insert_test_instrument(&db, "fictional-instrument-a").await;
    insert_test_instrument(&db, "fictional-instrument-b").await;

    upsert_bars(
        &db,
        vec![
            make_test_bar(
                "fictional-instrument-a",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
                100,
            ),
            make_test_bar(
                "fictional-instrument-a",
                NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"),
                105,
            ),
            make_test_bar(
                "fictional-instrument-b",
                NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
                200,
            ),
        ],
    )
    .await
    .expect("seed bars");

    let server = super::super::tests_common::build_server(db.clone());
    (db, server, strategy_id)
}

async fn fetch_evidence_by_step(
    db: &impl sea_orm::ConnectionTrait,
    execution_step_id: Uuid,
) -> Vec<strategy_task_step_evidence::Model> {
    let mut rows = strategy_task_step_evidence::Entity::find()
        .filter(strategy_task_step_evidence::Column::ExecutionStepId.eq(execution_step_id))
        .all(db)
        .await
        .expect("fetch evidence");
    rows.sort_by(|a, b| a.source_ref.cmp(&b.source_ref));
    rows
}

fn normalize_evidence_rows(
    rows: Vec<strategy_task_step_evidence::Model>,
) -> Vec<strategy_task_step_evidence::Model> {
    rows.into_iter()
        .map(|mut row| {
            row.id = Uuid::nil();
            row.observed_at =
                DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z").expect("timestamp sentinel");
            row
        })
        .collect()
}

#[backend_test_macros::database_test]
async fn query_data_returns_bars_for_each_requested_instrument(
    db: gateway_postgres::DatabaseHandle,
) {
    let (_db, server, strategy_id) = setup_server_with_bars(db).await;

    let result = server
        .query_data(
            strategy_id,
            None,
            QueryDataParams {
                instrument_ids: vec![
                    "fictional-instrument-a".into(),
                    "fictional-instrument-b".into(),
                ],
                from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                timeframe: None,
            },
        )
        .await
        .expect("query");

    assert_eq!(
        result,
        QueryDataResult {
            results: vec![
                InstrumentBarsDto {
                    instrument_id: "fictional-instrument-a".to_string(),
                    bars: vec![
                        bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 100),
                        bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105),
                    ],
                },
                InstrumentBarsDto {
                    instrument_id: "fictional-instrument-b".to_string(),
                    bars: vec![bar_dto(
                        NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
                        200
                    )],
                },
            ],
        },
    );
}

#[backend_test_macros::database_test]
async fn query_data_returns_minute_and_aggregated_intraday_bars(
    db: gateway_postgres::DatabaseHandle,
) {
    let (db, server, strategy_id) = setup_server_with_bars(db).await;
    insert_test_instrument_with_market(&db, "fictional-us-instrument-a", "US").await;
    insert_test_instrument_with_market(&db, "fictional-us-instrument-b", "US").await;
    insert_test_minute_bars(&db, "fictional-us-instrument-a", 100).await;
    insert_test_minute_bars(&db, "fictional-us-instrument-b", 200).await;

    for (timeframe, aggregate_bucket) in [
        ("1m", None),
        ("5m", Some((14, 30))),
        ("15m", Some((14, 30))),
        ("1h", Some((14, 0))),
        ("4h", Some((12, 0))),
    ] {
        let result = server
            .query_data(
                strategy_id,
                None,
                QueryDataParams {
                    instrument_ids: vec![
                        "fictional-us-instrument-b".into(),
                        "fictional-instrument-a".into(),
                        "fictional-us-instrument-a".into(),
                    ],
                    from: NaiveDate::from_ymd_opt(2034, 1, 2).expect("from"),
                    to: NaiveDate::from_ymd_opt(2034, 1, 2).expect("to"),
                    timeframe: Some(timeframe.to_string()),
                },
            )
            .await
            .expect("query");

        let results = if let Some((hour, minute)) = aggregate_bucket {
            vec![
                InstrumentBarsDto {
                    instrument_id: "fictional-us-instrument-b".to_string(),
                    bars: vec![minute_bar_dto(hour, minute, 200, 205, 199, 202, 30)],
                },
                InstrumentBarsDto {
                    instrument_id: "fictional-instrument-a".to_string(),
                    bars: vec![],
                },
                InstrumentBarsDto {
                    instrument_id: "fictional-us-instrument-a".to_string(),
                    bars: vec![minute_bar_dto(hour, minute, 100, 105, 99, 102, 30)],
                },
            ]
        } else {
            vec![
                InstrumentBarsDto {
                    instrument_id: "fictional-us-instrument-b".to_string(),
                    bars: vec![
                        minute_bar_dto(14, 30, 200, 205, 199, 201, 10),
                        minute_bar_dto(14, 31, 201, 204, 200, 202, 20),
                    ],
                },
                InstrumentBarsDto {
                    instrument_id: "fictional-instrument-a".to_string(),
                    bars: vec![],
                },
                InstrumentBarsDto {
                    instrument_id: "fictional-us-instrument-a".to_string(),
                    bars: vec![
                        minute_bar_dto(14, 30, 100, 105, 99, 101, 10),
                        minute_bar_dto(14, 31, 101, 104, 100, 102, 20),
                    ],
                },
            ]
        };

        assert_eq!(result, QueryDataResult { results });
    }
}

#[backend_test_macros::database_test]
async fn query_data_returns_empty_bars_for_instrument_with_no_data(
    db: gateway_postgres::DatabaseHandle,
) {
    let (_db, server, strategy_id) = setup_server_with_bars(db).await;

    let result = server
        .query_data(
            strategy_id,
            None,
            QueryDataParams {
                instrument_ids: vec![
                    "fictional-instrument-a".into(),
                    "fictional-instrument-missing".into(),
                ],
                from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                timeframe: None,
            },
        )
        .await
        .expect("query");

    assert_eq!(
        result,
        QueryDataResult {
            results: vec![
                InstrumentBarsDto {
                    instrument_id: "fictional-instrument-a".to_string(),
                    bars: vec![
                        bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 100),
                        bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105),
                    ],
                },
                InstrumentBarsDto {
                    instrument_id: "fictional-instrument-missing".to_string(),
                    bars: vec![],
                },
            ],
        },
    );
}

#[backend_test_macros::database_test]
async fn query_data_records_evidence_per_instrument_when_execution_step_id_present(
    db: gateway_postgres::DatabaseHandle,
) {
    let (db, server, strategy_id) = setup_server_with_bars(db).await;
    let execution_step_id = Uuid::new_v4();

    server
        .query_data(
            strategy_id,
            Some(execution_step_id),
            QueryDataParams {
                instrument_ids: vec![
                    "fictional-instrument-a".into(),
                    "fictional-instrument-b".into(),
                ],
                from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                timeframe: None,
            },
        )
        .await
        .expect("query");

    assert_eq!(
        normalize_evidence_rows(fetch_evidence_by_step(&db, execution_step_id).await),
        vec![
            strategy_task_step_evidence::Model {
                id: Uuid::nil(),
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: "fictional-instrument-a".to_string(),
                observed_at: DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
                    .expect("timestamp sentinel"),
                published_at: Some(
                    bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105).timestamp
                ),
                effective_at: Some(
                    bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105).timestamp
                ),
                snapshot: json!({
                    "instrument_id": "fictional-instrument-a",
                    "from": NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    "to": NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                    "bars": [
                        bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 100),
                        bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105),
                    ],
                    "total_bars": 2,
                    "truncated": false,
                }),
            },
            strategy_task_step_evidence::Model {
                id: Uuid::nil(),
                execution_step_id,
                source: "query_data".to_string(),
                source_ref: "fictional-instrument-b".to_string(),
                observed_at: DateTime::parse_from_rfc3339("2000-01-01T00:00:00Z")
                    .expect("timestamp sentinel"),
                published_at: Some(
                    bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 200).timestamp
                ),
                effective_at: Some(
                    bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 200).timestamp
                ),
                snapshot: json!({
                    "instrument_id": "fictional-instrument-b",
                    "from": NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    "to": NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                    "bars": [bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 200)],
                    "total_bars": 1,
                    "truncated": false,
                }),
            },
        ]
    );
}

#[backend_test_macros::database_test]
async fn query_data_records_no_evidence_when_execution_step_id_absent(
    db: gateway_postgres::DatabaseHandle,
) {
    let (db, server, strategy_id) = setup_server_with_bars(db).await;

    server
        .query_data(
            strategy_id,
            None,
            QueryDataParams {
                instrument_ids: vec!["fictional-instrument-a".into()],
                from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                timeframe: None,
            },
        )
        .await
        .expect("query");

    let rows = strategy_task_step_evidence::Entity::find()
        .all(&db)
        .await
        .expect("fetch evidence");
    assert_eq!(rows, Vec::new());
}

#[rstest]
#[case::empty_instrument_ids(
        vec![],
        "2025-01-06",
        "2025-01-07",
        "instrument_ids must not be empty"
    )]
#[case::too_many_instrument_ids(
        (0..MAX_QUERY_DATA_INSTRUMENTS + 1).map(|i| i.to_string()).collect(),
        "2025-01-06",
        "2025-01-07",
        "instrument_ids must not exceed 100 entries"
    )]
#[case::blank_instrument_id(
        vec!["  ".to_string()],
        "2025-01-06",
        "2025-01-07",
        "instrument_ids must not contain empty values"
    )]
#[case::duplicate_instrument_id(
        vec!["fictional-instrument-a".to_string(), "fictional-instrument-a".to_string()],
        "2025-01-06",
        "2025-01-07",
        "instrument_ids must not contain duplicates"
    )]
#[case::from_after_to(
        vec!["fictional-instrument-a".to_string()],
        "2025-01-07",
        "2025-01-06",
        "from must be on or before to"
    )]
#[tokio::test]
async fn query_data_rejects_invalid_params_through_tool_dispatch(
    #[case] instrument_ids: Vec<String>,
    #[case] from: &str,
    #[case] to: &str,
    #[case] expected_message: &str,
) {
    let strategy_id = Uuid::new_v4();
    let server = super::super::tests_common::build_server(mock_db_with_strategy(strategy_id));
    let err = server
        .query_data(
            strategy_id,
            None,
            QueryDataParams {
                instrument_ids,
                from: from.parse().expect("from date"),
                to: to.parse().expect("to date"),
                timeframe: None,
            },
        )
        .await
        .expect_err("expected invalid params");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params(expected_message.to_string(), None),
    );
}
