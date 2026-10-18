use chrono::FixedOffset;
use core_application::bars::{BarsRepositoryError, UsStockBarTarget};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use super::{PostgresBarsRepository, repository_error, transaction_ref};

impl PostgresBarsRepository {
    pub(super) async fn find_pending_us_stock_sip_bar_refetch_targets(
        &self,
        max_symbols: Option<usize>,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
        #[derive(FromQueryResult)]
        struct TargetRow {
            instrument_id: String,
            latest_daily_bar: Option<chrono::DateTime<FixedOffset>>,
            latest_minute_bar: Option<chrono::DateTime<FixedOffset>>,
            latest_hourly_bar: Option<chrono::DateTime<FixedOffset>>,
            hourly_history_completed_through: Option<chrono::DateTime<FixedOffset>>,
            earliest_daily_bar: Option<chrono::DateTime<FixedOffset>>,
            earliest_minute_bar: Option<chrono::DateTime<FixedOffset>>,
            earliest_hourly_bar: Option<chrono::DateTime<FixedOffset>>,
        }

        let query = r#"SELECT
                instruments.id AS instrument_id,
                daily_coverage.latest_timestamp AS latest_daily_bar,
                minute_coverage.latest_timestamp AS latest_minute_bar,
                hourly_coverage.latest_timestamp AS latest_hourly_bar,
                hourly_progress.completed_through AS hourly_history_completed_through,
                daily_coverage.earliest_timestamp AS earliest_daily_bar,
                minute_coverage.earliest_timestamp AS earliest_minute_bar,
                hourly_coverage.earliest_timestamp AS earliest_hourly_bar
            FROM instruments
            LEFT JOIN bars_timestamp_coverage AS daily_coverage
                ON daily_coverage.instrument_id = instruments.id
               AND daily_coverage.timeframe = '1d'
            LEFT JOIN bars_timestamp_coverage AS minute_coverage
                ON minute_coverage.instrument_id = instruments.id
               AND minute_coverage.timeframe = '1m'
            LEFT JOIN bars_timestamp_coverage AS hourly_coverage
                ON hourly_coverage.instrument_id = instruments.id
               AND hourly_coverage.timeframe = '1h'
            LEFT JOIN us_stock_hourly_bar_ingest_progress AS hourly_progress
                ON hourly_progress.instrument_id = instruments.id
            WHERE instruments.market = 'US'
              AND (
                  daily_coverage.instrument_id IS NOT NULL
                  OR minute_coverage.instrument_id IS NOT NULL
                  OR hourly_coverage.instrument_id IS NOT NULL
              )
              AND NOT EXISTS (
                  SELECT 1 FROM us_stock_sip_bar_refetch_progress AS progress
                  WHERE progress.instrument_id = instruments.id
              )
            ORDER BY instruments.id
            LIMIT $1"#;
        let limit = max_symbols.map(|limit| i64::try_from(limit).unwrap_or(i64::MAX));
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                query,
                [limit.into()],
            ))
            .await
            .map_err(repository_error)?;

        rows.iter()
            .map(|row| {
                let row = TargetRow::from_query_result(row, "").map_err(repository_error)?;
                Ok(UsStockBarTarget {
                    instrument_id: row.instrument_id,
                    latest_daily_bar: row.latest_daily_bar.map(|timestamp| timestamp.to_utc()),
                    latest_minute_bar: row.latest_minute_bar.map(|timestamp| timestamp.to_utc()),
                    latest_hourly_bar: row.latest_hourly_bar.map(|timestamp| timestamp.to_utc()),
                    hourly_history_completed_through: row
                        .hourly_history_completed_through
                        .map(|timestamp| timestamp.to_utc()),
                    earliest_daily_bar: row.earliest_daily_bar.map(|timestamp| timestamp.to_utc()),
                    earliest_minute_bar: row
                        .earliest_minute_bar
                        .map(|timestamp| timestamp.to_utc()),
                    earliest_hourly_bar: row
                        .earliest_hourly_bar
                        .map(|timestamp| timestamp.to_utc()),
                })
            })
            .collect()
    }

    pub(super) async fn mark_us_stock_sip_bar_refetch_completed(
        &self,
        instrument_id: &str,
    ) -> Result<(), BarsRepositoryError> {
        self.db
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO us_stock_sip_bar_refetch_progress \
                 (instrument_id, completed_at, skipped_reason) \
                 VALUES ($1, CURRENT_TIMESTAMP, NULL) \
                 ON CONFLICT (instrument_id) DO UPDATE \
                 SET completed_at = EXCLUDED.completed_at, skipped_reason = NULL",
                [instrument_id.into()],
            ))
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    pub(super) async fn mark_us_stock_sip_bar_refetch_skipped(
        &self,
        instrument_id: &str,
        reason: &str,
    ) -> Result<(), BarsRepositoryError> {
        self.db
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO us_stock_sip_bar_refetch_progress \
                 (instrument_id, completed_at, skipped_reason) \
                 VALUES ($1, NULL, $2) \
                 ON CONFLICT (instrument_id) DO UPDATE \
                 SET completed_at = NULL, skipped_reason = EXCLUDED.skipped_reason",
                [instrument_id.into(), reason.into()],
            ))
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    pub(super) async fn reserve_us_stock_sip_refetch_rows(
        &self,
        transaction: &UnitOfWorkTransaction,
        rows: u64,
        max_rows_per_hour: u64,
    ) -> Result<bool, BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let rows = i64::try_from(rows).unwrap_or(i64::MAX);
        let max_rows_per_hour = i64::try_from(max_rows_per_hour).unwrap_or(i64::MAX);
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO us_stock_sip_bar_refetch_write_budget AS budget \
                 (id, hour_started_at, rows_written) \
                 VALUES (1, date_trunc('hour', statement_timestamp() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC', $1) \
                 ON CONFLICT (id) DO UPDATE \
                 SET hour_started_at = EXCLUDED.hour_started_at, \
                     rows_written = CASE \
                         WHEN budget.hour_started_at = EXCLUDED.hour_started_at \
                         THEN budget.rows_written + EXCLUDED.rows_written \
                         ELSE EXCLUDED.rows_written \
                     END \
                 WHERE CASE \
                     WHEN budget.hour_started_at = EXCLUDED.hour_started_at \
                     THEN budget.rows_written + EXCLUDED.rows_written <= $2 \
                     ELSE EXCLUDED.rows_written <= $2 \
                 END \
                 RETURNING rows_written",
                [rows.into(), max_rows_per_hour.into()],
            ))
            .await
            .map_err(repository_error)?;
        if row.is_none() {
            return Ok(false);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use core_application::{
        bars::{BarsRepository, UsStockBarTarget},
        unit_of_work::UnitOfWork,
    };
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;
    use sea_orm::{ConnectionTrait, EntityTrait, Set, Statement};

    use crate::{DatabaseHandle, PostgresUnitOfWork, entities::instruments};

    use super::PostgresBarsRepository;

    fn bar(instrument_id: &str, timeframe: Timeframe) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp: Utc
                .with_ymd_and_hms(2040, 1, 1, 0, 0, 0)
                .single()
                .unwrap_or_default(),
            open: Decimal::new(101, 1),
            high: Decimal::new(111, 1),
            low: Decimal::new(91, 1),
            close: Decimal::new(105, 1),
            volume: 25,
            adjustment_factor: Decimal::ONE,
        }
    }

    async fn insert_instruments(
        repository: &PostgresBarsRepository,
        db: &DatabaseHandle,
        instruments: &[(&str, &str)],
    ) {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        instruments::Entity::insert_many(instruments.iter().map(|(instrument_id, market)| {
            instruments::ActiveModel {
                id: Set((*instrument_id).to_owned()),
                name: Set((*instrument_id).to_owned()),
                market: Set((*market).to_owned()),
                sector: Set(None),
            }
        }))
        .exec_without_returning(
            transaction
                .downcast_ref::<sea_orm::DatabaseTransaction>()
                .expect("postgres transaction"),
        )
        .await
        .expect("insert instruments");
        let bars = instruments
            .iter()
            .filter(|(instrument_id, _)| *instrument_id != "US:DEMO-REFETCH-EMPTY")
            .flat_map(|(instrument_id, _)| {
                [Timeframe::Daily, Timeframe::Hourly, Timeframe::Minute]
                    .map(|timeframe| bar(instrument_id, timeframe))
            })
            .collect();
        repository
            .upsert_bars(&transaction, bars)
            .await
            .expect("insert bars");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit instrument transaction");
    }

    #[backend_test_macros::database_test]
    async fn pending_refetch_targets_are_limited_and_exclude_finished_or_empty_symbols(
        db: DatabaseHandle,
    ) {
        let repository = PostgresBarsRepository::new(db.clone());
        insert_instruments(
            &repository,
            &db,
            &[
                ("US:DEMO-REFETCH-ALPHA", "US"),
                ("US:DEMO-REFETCH-BETA", "US"),
                ("US:DEMO-REFETCH-GAMMA", "US"),
                ("US:DEMO-REFETCH-DELTA", "US"),
                ("US:DEMO-REFETCH-EMPTY", "US"),
                ("DEMO-REFETCH-DOMESTIC", "TSE"),
            ],
        )
        .await;
        repository
            .mark_us_stock_sip_bar_refetch_completed("US:DEMO-REFETCH-BETA")
            .await
            .expect("mark completed refetch");
        repository
            .mark_us_stock_sip_bar_refetch_skipped(
                "US:DEMO-REFETCH-GAMMA",
                "synthetic instrument rejection",
            )
            .await
            .expect("mark skipped refetch");

        let limited = repository
            .find_pending_us_stock_sip_bar_refetch_targets(Some(1))
            .await
            .expect("find limited pending targets");
        let all = repository
            .find_pending_us_stock_sip_bar_refetch_targets(None)
            .await
            .expect("find all pending targets");

        assert_eq!(
            (limited, all),
            (
                vec![UsStockBarTarget {
                    instrument_id: "US:DEMO-REFETCH-ALPHA".to_owned(),
                    latest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                    latest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                    latest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                    hourly_history_completed_through: None,
                    earliest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                    earliest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                    earliest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                }],
                vec![
                    "US:DEMO-REFETCH-ALPHA".to_owned(),
                    "US:DEMO-REFETCH-DELTA".to_owned(),
                ]
                .into_iter()
                .map(|instrument_id| UsStockBarTarget {
                    instrument_id,
                    latest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                    latest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                    latest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                    hourly_history_completed_through: None,
                    earliest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                    earliest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                    earliest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                })
                .collect(),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn row_reservations_enforce_the_current_hour_and_reset_for_a_new_hour(
        db: DatabaseHandle,
    ) {
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let first_transaction = unit_of_work.begin().await.expect("begin transaction");
        let first_allowed = repository
            .reserve_us_stock_sip_refetch_rows(&first_transaction, 3, 5)
            .await
            .expect("reserve first rows");
        let over_limit_allowed = repository
            .reserve_us_stock_sip_refetch_rows(&first_transaction, 3, 5)
            .await
            .expect("reject rows over limit");
        unit_of_work
            .commit(first_transaction)
            .await
            .expect("commit first reservation");
        db.execute_unprepared(
            "UPDATE us_stock_sip_bar_refetch_write_budget \
             SET hour_started_at = hour_started_at - INTERVAL '1 hour'",
        )
        .await
        .expect("move rate-limit window to the previous hour");
        let next_hour_transaction = unit_of_work.begin().await.expect("begin next hour");
        let next_hour_allowed = repository
            .reserve_us_stock_sip_refetch_rows(&next_hour_transaction, 5, 5)
            .await
            .expect("reserve rows in the next hour");
        unit_of_work
            .commit(next_hour_transaction)
            .await
            .expect("commit next-hour reservation");
        let budget = db
            .query_one_raw(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT rows_written FROM us_stock_sip_bar_refetch_write_budget WHERE id = 1",
            ))
            .await
            .expect("read row budget")
            .map(|row| {
                row.try_get::<i64>("", "rows_written")
                    .expect("read rows_written")
            });

        assert_eq!(
            (first_allowed, over_limit_allowed, next_hour_allowed, budget),
            (true, false, true, Some(5)),
        );
    }
}
