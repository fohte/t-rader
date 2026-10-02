use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rstest::rstest;
use rust_decimal::Decimal;
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, DatabaseBackend, DatabaseConnection, EntityTrait, MockDatabase, QueryFilter, Set,
};
use serde_json::json;
use uuid::Uuid;

use super::super::StrategyServer;
use super::super::dto::{BarDto, InstrumentBarsDto, QueryDataParams, QueryDataResult};
use super::super::tests_common::insert_strategy;
use super::MAX_QUERY_DATA_INSTRUMENTS;
use crate::models::Bar;
use crate::models::bar::Timeframe;
use gateway_postgres::entities::{instruments, strategy_task_step_evidence};
use gateway_postgres::repositories::bars::upsert_bars;

fn mock_db(strategy_id: Uuid) -> DatabaseConnection {
    let row =
        std::collections::BTreeMap::from([("id".to_string(), sea_orm::Value::from(strategy_id))]);
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row]])
        .into_connection()
}

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
    let server = super::super::tests_common::build_server(mock_db(strategy_id));
    let err = server
        .query_data(
            strategy_id,
            None,
            QueryDataParams {
                instrument_ids,
                from: from.parse().expect("from date"),
                to: to.parse().expect("to date"),
            },
        )
        .await
        .expect_err("expected invalid params");
    assert_eq!(
        err,
        rmcp::ErrorData::invalid_params(expected_message.to_string(), None),
    );
}
