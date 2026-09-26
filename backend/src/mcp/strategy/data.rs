//! 価格データ取得の inner method 実装。
//!
//! DB (`repositories::bars`) から複数銘柄分のバーデータをまとめて取得し、
//! MCP の wire 表現 ([`InstrumentBarsDto`]) に変換する。日足は全上場銘柄分が
//! `services::daily_bars_ingest` で定期的に取り込まれているため、ここでは
//! データプロバイダへの問い合わせは行わない (呼び出しのたびに叩くとレート制限に
//! 当たるため)。

use std::collections::HashMap;

use rmcp::ErrorData as McpError;
use uuid::Uuid;

use crate::repositories::bars::find_bars_by_instruments;

use super::dto::{BarDto, InstrumentBarsDto, QueryDataParams, QueryDataResult};
use super::{StrategyServer, app_error_to_mcp, decimal_to_f64, invalid_params};

/// 1 回の呼び出しで指定できる銘柄数の上限
const MAX_QUERY_DATA_INSTRUMENTS: usize = 100;

impl StrategyServer {
    pub(crate) async fn query_data_inner(
        &self,
        _session_strategy_id: Uuid,
        execution_step_id: Option<Uuid>,
        params: QueryDataParams,
    ) -> Result<QueryDataResult, McpError> {
        if params.instrument_ids.is_empty() {
            return Err(invalid_params("instrument_ids must not be empty"));
        }
        if params.instrument_ids.len() > MAX_QUERY_DATA_INSTRUMENTS {
            return Err(invalid_params(format!(
                "instrument_ids must not exceed {MAX_QUERY_DATA_INSTRUMENTS} entries"
            )));
        }
        if params.from > params.to {
            return Err(invalid_params("from must be on or before to"));
        }

        let instrument_ids: Vec<String> = params
            .instrument_ids
            .iter()
            .map(|id| id.trim().to_string())
            .collect();
        if instrument_ids.iter().any(String::is_empty) {
            return Err(invalid_params(
                "instrument_ids must not contain empty values",
            ));
        }
        {
            let mut seen = std::collections::HashSet::with_capacity(instrument_ids.len());
            if !instrument_ids.iter().all(|id| seen.insert(id)) {
                return Err(invalid_params("instrument_ids must not contain duplicates"));
            }
        }

        let from = params
            .from
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().fixed_offset());
        let to = params
            .to
            .and_hms_opt(23, 59, 59)
            .map(|dt| dt.and_utc().fixed_offset());

        let rows = find_bars_by_instruments(&self.db, &instrument_ids, "1d", from, to)
            .await
            .map_err(app_error_to_mcp)?;

        let mut bars_by_instrument: HashMap<String, Vec<BarDto>> = HashMap::new();
        for row in rows {
            bars_by_instrument
                .entry(row.instrument_id.clone())
                .or_default()
                .push(BarDto {
                    timestamp: row.timestamp,
                    open: decimal_to_f64(row.open),
                    high: decimal_to_f64(row.high),
                    low: decimal_to_f64(row.low),
                    close: decimal_to_f64(row.close),
                    volume: row.volume,
                });
        }

        let mut results = Vec::with_capacity(instrument_ids.len());
        for instrument_id in &instrument_ids {
            let bars = bars_by_instrument.remove(instrument_id).unwrap_or_default();

            if let Some(execution_step_id) = execution_step_id
                && let Err(err) = super::evidence::record_query_data(
                    &self.db,
                    execution_step_id,
                    instrument_id,
                    params.from,
                    params.to,
                    &bars,
                )
                .await
            {
                tracing::warn!(
                    error = %err,
                    %execution_step_id,
                    %instrument_id,
                    "failed to record query_data evidence",
                );
            }

            results.push(InstrumentBarsDto {
                instrument_id: instrument_id.clone(),
                bars,
            });
        }

        Ok(QueryDataResult { results })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ColumnTrait, DatabaseBackend, DatabaseConnection, EntityTrait, MockDatabase, QueryFilter,
        Set,
    };
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::entities::{instruments, strategy_task_step_evidence};
    use crate::models::Bar;
    use crate::models::bar::Timeframe;
    use crate::repositories::bars::upsert_bars;
    use crate::testing::create_test_db;

    use super::super::StrategyServer;
    use super::super::dto::{BarDto, InstrumentBarsDto, QueryDataParams, QueryDataResult};
    use super::super::tests_common::insert_strategy;
    use super::MAX_QUERY_DATA_INSTRUMENTS;

    fn mock_db() -> DatabaseConnection {
        MockDatabase::new(DatabaseBackend::Postgres).into_connection()
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
        pool: crate::database::DatabaseHandle,
    ) -> (crate::database::DatabaseHandle, StrategyServer, Uuid) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;

        insert_test_instrument(&db, "7203").await;
        insert_test_instrument(&db, "9984").await;

        upsert_bars(
            &db,
            vec![
                make_test_bar(
                    "7203",
                    NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
                    100,
                ),
                make_test_bar(
                    "7203",
                    NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"),
                    105,
                ),
                make_test_bar(
                    "9984",
                    NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"),
                    200,
                ),
            ],
        )
        .await
        .expect("seed bars");

        let server = StrategyServer::new(db.clone(), None);
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

    #[backend_test_macros::database_test]
    async fn query_data_returns_bars_for_each_requested_instrument(pool: PgPool) {
        let (_db, server, strategy_id) = setup_server_with_bars(pool).await;

        let result = server
            .query_data_inner(
                strategy_id,
                None,
                QueryDataParams {
                    instrument_ids: vec!["7203".into(), "9984".into()],
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
                        instrument_id: "7203".to_string(),
                        bars: vec![
                            bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 100),
                            bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105),
                        ],
                    },
                    InstrumentBarsDto {
                        instrument_id: "9984".to_string(),
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
    async fn query_data_returns_empty_bars_for_instrument_with_no_data(pool: PgPool) {
        let (_db, server, strategy_id) = setup_server_with_bars(pool).await;

        let result = server
            .query_data_inner(
                strategy_id,
                None,
                QueryDataParams {
                    instrument_ids: vec!["7203".into(), "0000".into()],
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
                        instrument_id: "7203".to_string(),
                        bars: vec![
                            bar_dto(NaiveDate::from_ymd_opt(2025, 1, 6).expect("date"), 100),
                            bar_dto(NaiveDate::from_ymd_opt(2025, 1, 7).expect("date"), 105),
                        ],
                    },
                    InstrumentBarsDto {
                        instrument_id: "0000".to_string(),
                        bars: vec![],
                    },
                ],
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn query_data_records_evidence_per_instrument_when_execution_step_id_present(
        pool: PgPool,
    ) {
        let (db, server, strategy_id) = setup_server_with_bars(pool).await;
        let execution_step_id = Uuid::new_v4();

        server
            .query_data_inner(
                strategy_id,
                Some(execution_step_id),
                QueryDataParams {
                    instrument_ids: vec!["7203".into(), "9984".into()],
                    from: NaiveDate::from_ymd_opt(2025, 1, 6).expect("from"),
                    to: NaiveDate::from_ymd_opt(2025, 1, 7).expect("to"),
                },
            )
            .await
            .expect("query");

        let rows = fetch_evidence_by_step(&db, execution_step_id).await;
        let source_refs: Vec<String> = rows.into_iter().map(|r| r.source_ref).collect();
        assert_eq!(source_refs, vec!["7203".to_string(), "9984".to_string()]);
    }

    #[backend_test_macros::database_test]
    async fn query_data_records_no_evidence_when_execution_step_id_absent(pool: PgPool) {
        let (db, server, strategy_id) = setup_server_with_bars(pool).await;

        server
            .query_data_inner(
                strategy_id,
                None,
                QueryDataParams {
                    instrument_ids: vec!["7203".into()],
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
        vec!["7203".to_string(), "7203".to_string()],
        "2025-01-06",
        "2025-01-07",
        "instrument_ids must not contain duplicates"
    )]
    #[case::from_after_to(
        vec!["7203".to_string()],
        "2025-01-07",
        "2025-01-06",
        "from must be on or before to"
    )]
    #[tokio::test]
    async fn query_data_inner_rejects_invalid_params(
        #[case] instrument_ids: Vec<String>,
        #[case] from: &str,
        #[case] to: &str,
        #[case] expected_message: &str,
    ) {
        let server = StrategyServer::new(mock_db(), None);
        let err = server
            .query_data_inner(
                Uuid::new_v4(),
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
            (err.code, err.message.as_ref()),
            (rmcp::model::ErrorCode::INVALID_PARAMS, expected_message),
        );
    }
}
