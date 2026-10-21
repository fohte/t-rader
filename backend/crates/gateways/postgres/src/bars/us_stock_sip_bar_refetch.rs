use chrono::FixedOffset;
use core_application::bars::{
    BarsRepositoryError, UsStockBarTarget, UsStockSipBarRefetchCursor, UsStockSipBarRefetchTarget,
    UsStockSipBarRefetchTimeframe,
};
use core_application::unit_of_work::UnitOfWorkTransaction;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};

use super::{PostgresBarsRepository, repository_error, transaction_ref};

impl PostgresBarsRepository {
    pub(super) async fn find_pending_us_stock_sip_bar_refetch_targets(
        &self,
        max_symbols: Option<usize>,
    ) -> Result<Vec<UsStockSipBarRefetchTarget>, BarsRepositoryError> {
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
            resume_timeframe: Option<String>,
            resume_from: Option<chrono::DateTime<FixedOffset>>,
            resume_to: Option<chrono::DateTime<FixedOffset>>,
            resume_page_token: Option<String>,
        }

        let query = r#"SELECT
                instruments.id AS instrument_id,
                daily_coverage.latest_timestamp AS latest_daily_bar,
                minute_coverage.latest_timestamp AS latest_minute_bar,
                hourly_coverage.latest_timestamp AS latest_hourly_bar,
                hourly_progress.completed_through AS hourly_history_completed_through,
                daily_coverage.earliest_timestamp AS earliest_daily_bar,
                minute_coverage.earliest_timestamp AS earliest_minute_bar,
                hourly_coverage.earliest_timestamp AS earliest_hourly_bar,
                refetch_progress.resume_timeframe,
                refetch_progress.resume_from,
                refetch_progress.resume_to,
                refetch_progress.resume_page_token
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
            LEFT JOIN us_stock_sip_bar_refetch_progress AS refetch_progress
                ON refetch_progress.instrument_id = instruments.id
            WHERE instruments.market = 'US'
              AND (
                  daily_coverage.instrument_id IS NOT NULL
                  OR minute_coverage.instrument_id IS NOT NULL
                  OR hourly_coverage.instrument_id IS NOT NULL
              )
              AND (
                  refetch_progress.instrument_id IS NULL
                  OR (
                      refetch_progress.completed_at IS NULL
                      AND refetch_progress.skipped_reason IS NULL
                  )
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
                let cursor = match row.resume_timeframe {
                    Some(timeframe) => {
                        let timeframe = timeframe
                            .parse::<UsStockSipBarRefetchTimeframe>()
                            .map_err(|error| {
                                BarsRepositoryError::Database(
                                    core_application::persistence::PersistenceError::Database(
                                        error,
                                    ),
                                )
                            })?;
                        let range_from = row.resume_from.ok_or_else(|| {
                            BarsRepositoryError::Database(
                                core_application::persistence::PersistenceError::Database(
                                    "refetch cursor has no range start".to_owned(),
                                ),
                            )
                        })?;
                        let range_to = row.resume_to.ok_or_else(|| {
                            BarsRepositoryError::Database(
                                core_application::persistence::PersistenceError::Database(
                                    "refetch cursor has no range end".to_owned(),
                                ),
                            )
                        })?;
                        Some(UsStockSipBarRefetchCursor {
                            timeframe,
                            range_from: range_from.to_utc(),
                            range_to: range_to.to_utc(),
                            page_token: row.resume_page_token.clone(),
                        })
                    }
                    None => None,
                };
                Ok(UsStockSipBarRefetchTarget {
                    bars: UsStockBarTarget {
                        instrument_id: row.instrument_id,
                        latest_daily_bar: row.latest_daily_bar.map(|timestamp| timestamp.to_utc()),
                        latest_minute_bar: row
                            .latest_minute_bar
                            .map(|timestamp| timestamp.to_utc()),
                        latest_hourly_bar: row
                            .latest_hourly_bar
                            .map(|timestamp| timestamp.to_utc()),
                        minute_history_completed_through: None,
                        hourly_history_completed_through: row
                            .hourly_history_completed_through
                            .map(|timestamp| timestamp.to_utc()),
                        earliest_daily_bar: row
                            .earliest_daily_bar
                            .map(|timestamp| timestamp.to_utc()),
                        earliest_minute_bar: row
                            .earliest_minute_bar
                            .map(|timestamp| timestamp.to_utc()),
                        earliest_hourly_bar: row
                            .earliest_hourly_bar
                            .map(|timestamp| timestamp.to_utc()),
                    },
                    cursor,
                })
            })
            .collect()
    }

    pub(super) async fn save_us_stock_sip_bar_refetch_cursor(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_id: &str,
        cursor: Option<UsStockSipBarRefetchCursor>,
    ) -> Result<(), BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        let timeframe = cursor.as_ref().map(|cursor| cursor.timeframe.as_str());
        let page_token = cursor
            .as_ref()
            .and_then(|cursor| cursor.page_token.as_deref());
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO us_stock_sip_bar_refetch_progress \
                 (instrument_id, completed_at, skipped_reason, resume_timeframe, resume_from, resume_to, resume_page_token) \
                 VALUES ($1, CASE WHEN $2::text IS NULL THEN CURRENT_TIMESTAMP ELSE NULL END, NULL, $2, $3, $4, $5) \
                 ON CONFLICT (instrument_id) DO UPDATE \
                 SET completed_at = CASE WHEN EXCLUDED.resume_timeframe IS NULL THEN CURRENT_TIMESTAMP ELSE NULL END, \
                     skipped_reason = NULL, \
                     resume_timeframe = EXCLUDED.resume_timeframe, \
                     resume_from = EXCLUDED.resume_from, \
                     resume_to = EXCLUDED.resume_to, \
                     resume_page_token = EXCLUDED.resume_page_token",
                [
                    instrument_id.into(),
                    timeframe.into(),
                    cursor.as_ref().map(|cursor| cursor.range_from).into(),
                    cursor.as_ref().map(|cursor| cursor.range_to).into(),
                    page_token.into(),
                ],
            ))
            .await
            .map(|_| ())
            .map_err(repository_error)
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
                 SET completed_at = EXCLUDED.completed_at, skipped_reason = NULL, \
                     resume_timeframe = NULL, resume_from = NULL, resume_to = NULL, resume_page_token = NULL",
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
                 SET completed_at = NULL, skipped_reason = EXCLUDED.skipped_reason, \
                     resume_timeframe = NULL, resume_from = NULL, resume_to = NULL, resume_page_token = NULL",
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
                 SELECT 1, date_trunc('hour', statement_timestamp() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC', $1 \
                 WHERE $1 <= $2 \
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
        bars::{
            BarsRepository, UsStockBarTarget, UsStockSipBarRefetchCursor,
            UsStockSipBarRefetchTarget, UsStockSipBarRefetchTimeframe,
        },
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
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work
            .begin()
            .await
            .expect("begin cursor transaction");
        repository
            .save_us_stock_sip_bar_refetch_cursor(
                &transaction,
                "US:DEMO-REFETCH-DELTA",
                Some(UsStockSipBarRefetchCursor {
                    timeframe: UsStockSipBarRefetchTimeframe::Minute,
                    range_from: bar("", Timeframe::Minute).timestamp,
                    range_to: bar("", Timeframe::Minute).timestamp,
                    page_token: Some("synthetic-page-token".to_owned()),
                }),
            )
            .await
            .expect("save partial refetch cursor");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit cursor transaction");

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
                vec![UsStockSipBarRefetchTarget {
                    bars: UsStockBarTarget {
                        instrument_id: "US:DEMO-REFETCH-ALPHA".to_owned(),
                        latest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                        latest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                        latest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                        minute_history_completed_through: None,
                        hourly_history_completed_through: None,
                        earliest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                        earliest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                        earliest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                    },
                    cursor: None,
                }],
                vec![
                    "US:DEMO-REFETCH-ALPHA".to_owned(),
                    "US:DEMO-REFETCH-DELTA".to_owned(),
                ]
                .into_iter()
                .map(|instrument_id| UsStockSipBarRefetchTarget {
                    bars: UsStockBarTarget {
                        instrument_id: instrument_id.clone(),
                        latest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                        latest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                        latest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                        minute_history_completed_through: None,
                        hourly_history_completed_through: None,
                        earliest_daily_bar: Some(bar("", Timeframe::Daily).timestamp),
                        earliest_minute_bar: Some(bar("", Timeframe::Minute).timestamp),
                        earliest_hourly_bar: Some(bar("", Timeframe::Hourly).timestamp),
                    },
                    cursor: (instrument_id == "US:DEMO-REFETCH-DELTA").then(|| {
                        UsStockSipBarRefetchCursor {
                            timeframe: UsStockSipBarRefetchTimeframe::Minute,
                            range_from: bar("", Timeframe::Minute).timestamp,
                            range_to: bar("", Timeframe::Minute).timestamp,
                            page_token: Some("synthetic-page-token".to_owned()),
                        }
                    }),
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
        db.execute_unprepared("DELETE FROM us_stock_sip_bar_refetch_write_budget")
            .await
            .expect("clear prior row budget");
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let first_transaction = unit_of_work.begin().await.expect("begin transaction");
        let initial_over_limit_allowed = repository
            .reserve_us_stock_sip_refetch_rows(&first_transaction, 6, 5)
            .await
            .expect("reject first reservation over the row limit");
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
            (
                initial_over_limit_allowed,
                first_allowed,
                over_limit_allowed,
                next_hour_allowed,
                budget,
            ),
            (false, true, false, true, Some(5)),
        );
    }
}
