use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use core_application::bars::{
    BarsByInstrumentsQuery, BarsQuery, BarsRepository, BarsRepositoryError,
};
use core_application::persistence::PersistenceError;
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::bar::{Bar, Timeframe};
use core_domain::instrument::Market;
use sea_orm::sea_query::{Expr, OnConflict};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Set, Statement,
};

use crate::DatabaseHandle;
use crate::entities::{
    instruments, jquants_daily_bars_ingested_date, us_stock_hourly_bar_ingest_progress,
    us_stock_minute_bar_ingest_progress,
};
use crate::persistence::persistence_error;
use crate::repositories::bars::{self as bar_queries, BarsQuery as PostgresBarsQuery};
use crate::transaction::transaction_ref as postgres_transaction_ref;

mod us_stock_bar_targets;
mod us_stock_minute_bars_downsample;
mod us_stock_sip_bar_refetch;

fn current_hourly_history_cutoff_for(timeframe: Timeframe) -> Option<DateTime<FixedOffset>> {
    matches!(timeframe, Timeframe::Hourly | Timeframe::FourHours)
        .then(|| core_application::bars::us_stock_hourly_history_cutoff(Utc::now()).fixed_offset())
}

#[derive(Clone)]
pub struct PostgresBarsRepository {
    db: DatabaseHandle,
}

impl PostgresBarsRepository {
    pub fn new(db: DatabaseHandle) -> Self {
        Self { db }
    }

    async fn us_instrument_ids(
        &self,
        instrument_ids: &[String],
    ) -> Result<Vec<String>, BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(Vec::new());
        }

        instruments::Entity::find()
            .filter(instruments::Column::Id.is_in(instrument_ids.to_vec()))
            .filter(instruments::Column::Market.eq(Market::Us.to_string()))
            .all(&self.db)
            .await
            .map(|instruments| {
                instruments
                    .into_iter()
                    .map(|instrument| instrument.id)
                    .collect()
            })
            .map_err(repository_error)
    }
}

#[async_trait]
impl BarsRepository for PostgresBarsRepository {
    async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError> {
        let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
            return Ok(Vec::new());
        };

        if timeframe == Timeframe::Weekly {
            return bar_queries::find_weekly_bars_by_instruments(
                &self.db,
                std::slice::from_ref(&query.instrument_id),
                query.from,
                query.to,
            )
            .await
            .map_err(repository_error);
        }

        if timeframe != Timeframe::Daily {
            if self
                .us_instrument_ids(std::slice::from_ref(&query.instrument_id))
                .await?
                .is_empty()
            {
                return Ok(Vec::new());
            }

            let hourly_history_cutoff = current_hourly_history_cutoff_for(timeframe);
            return bar_queries::find_intraday_bars(
                &self.db,
                to_postgres_query(query),
                hourly_history_cutoff,
            )
            .await
            .map_err(repository_error);
        }

        bar_queries::find_bars(&self.db, to_postgres_query(query))
            .await
            .map(|rows| rows.into_iter().map(Into::into).collect())
            .map_err(repository_error)
    }

    async fn find_bars_by_instruments(
        &self,
        query: BarsByInstrumentsQuery,
    ) -> Result<Vec<Bar>, BarsRepositoryError> {
        let Ok(timeframe) = query.timeframe.parse::<Timeframe>() else {
            return Ok(Vec::new());
        };

        if timeframe != Timeframe::Daily {
            let us_instrument_ids = self.us_instrument_ids(&query.instrument_ids).await?;

            return bar_queries::find_intraday_bars_by_instruments(
                &self.db,
                &us_instrument_ids,
                timeframe,
                current_hourly_history_cutoff_for(timeframe),
                query.from,
                query.to,
            )
            .await
            .map_err(repository_error);
        }

        bar_queries::find_bars_by_instruments(
            &self.db,
            &query.instrument_ids,
            &query.timeframe,
            query.from,
            query.to,
        )
        .await
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(repository_error)
    }

    async fn find_latest_bar(
        &self,
        instrument_id: &str,
        timeframe: &str,
    ) -> Result<Option<Bar>, BarsRepositoryError> {
        bar_queries::find_latest_bar(&self.db, instrument_id, timeframe)
            .await
            .map(|row| row.map(Into::into))
            .map_err(repository_error)
    }

    async fn find_daily_adjustment_factors_from(
        &self,
        instrument_id: &str,
        from: NaiveDate,
    ) -> Result<Vec<core_application::bars::DailyBarAdjustmentFactor>, BarsRepositoryError> {
        bar_queries::find_daily_adjustment_factors_from(&self.db, instrument_id, from)
            .await
            .map_err(repository_error)
    }

    async fn find_us_stock_minute_bar_targets(
        &self,
    ) -> Result<Vec<core_application::bars::UsStockBarTarget>, BarsRepositoryError> {
        self.find_us_stock_bar_targets().await
    }

    async fn find_us_stock_daily_bar_targets(
        &self,
    ) -> Result<Vec<core_application::bars::UsStockBarTarget>, BarsRepositoryError> {
        self.find_us_stock_bar_targets().await
    }

    async fn find_pending_us_stock_sip_bar_refetch_targets(
        &self,
        max_symbols: Option<usize>,
    ) -> Result<Vec<core_application::bars::UsStockSipBarRefetchTarget>, BarsRepositoryError> {
        PostgresBarsRepository::find_pending_us_stock_sip_bar_refetch_targets(self, max_symbols)
            .await
    }

    async fn save_us_stock_sip_bar_refetch_cursor(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_id: &str,
        cursor: Option<core_application::bars::UsStockSipBarRefetchCursor>,
    ) -> Result<(), BarsRepositoryError> {
        PostgresBarsRepository::save_us_stock_sip_bar_refetch_cursor(
            self,
            transaction,
            instrument_id,
            cursor,
        )
        .await
    }

    async fn mark_us_stock_sip_bar_refetch_completed(
        &self,
        instrument_id: &str,
    ) -> Result<(), BarsRepositoryError> {
        PostgresBarsRepository::mark_us_stock_sip_bar_refetch_completed(self, instrument_id).await
    }

    async fn mark_us_stock_sip_bar_refetch_skipped(
        &self,
        instrument_id: &str,
        reason: &str,
    ) -> Result<(), BarsRepositoryError> {
        PostgresBarsRepository::mark_us_stock_sip_bar_refetch_skipped(self, instrument_id, reason)
            .await
    }

    async fn reserve_us_stock_sip_refetch_rows(
        &self,
        transaction: &UnitOfWorkTransaction,
        rows: u64,
        max_rows_per_hour: u64,
        paced_at: DateTime<Utc>,
        pacing_millis: u64,
    ) -> Result<bool, BarsRepositoryError> {
        PostgresBarsRepository::reserve_us_stock_sip_refetch_rows(
            self,
            transaction,
            rows,
            max_rows_per_hour,
            paced_at,
            pacing_millis,
        )
        .await
    }

    async fn find_us_stock_sip_refetch_next_allowed_at(
        &self,
    ) -> Result<Option<DateTime<Utc>>, BarsRepositoryError> {
        PostgresBarsRepository::find_us_stock_sip_refetch_next_allowed_at(self).await
    }

    async fn find_earliest_us_stock_minute_bar_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, BarsRepositoryError> {
        us_stock_minute_bars_downsample::find_earliest_minute_bar_before(&self.db, cutoff)
            .await
            .map_err(repository_error)
    }

    async fn find_us_stock_instrument_ids(&self) -> Result<Vec<String>, BarsRepositoryError> {
        instruments::Entity::find()
            .filter(instruments::Column::Market.eq(Market::Us.to_string()))
            .order_by_asc(instruments::Column::Id)
            .all(&self.db)
            .await
            .map(|instruments| {
                instruments
                    .into_iter()
                    .map(|instrument| instrument.id)
                    .collect()
            })
            .map_err(repository_error)
    }

    async fn downsample_us_stock_minute_bar_range(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_id: &str,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
        cutoff: DateTime<Utc>,
    ) -> Result<core_application::bars::UsStockMinuteBarsDownsampleStats, BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        us_stock_minute_bars_downsample::downsample_minute_bar_range(
            transaction,
            instrument_id,
            from,
            until,
            cutoff,
        )
        .await
        .map_err(repository_error)
    }

    async fn mark_us_stock_hourly_bar_history_completed(
        &self,
        instrument_ids: &[String],
        completed_through: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(());
        }

        us_stock_hourly_bar_ingest_progress::Entity::insert_many(
            instrument_ids.iter().map(|instrument_id| {
                us_stock_hourly_bar_ingest_progress::ActiveModel {
                    instrument_id: Set(instrument_id.clone()),
                    completed_through: Set(completed_through.fixed_offset()),
                }
            }),
        )
        .on_conflict(
            OnConflict::column(us_stock_hourly_bar_ingest_progress::Column::InstrumentId)
                .value(
                    us_stock_hourly_bar_ingest_progress::Column::CompletedThrough,
                    Expr::cust(
                        "GREATEST(us_stock_hourly_bar_ingest_progress.completed_through, EXCLUDED.completed_through)",
                    ),
                )
                .to_owned(),
        )
        .exec_without_returning(&self.db)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }

    async fn mark_us_stock_minute_bar_history_completed(
        &self,
        instrument_ids: &[String],
        completed_through: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(());
        }

        us_stock_minute_bar_ingest_progress::Entity::insert_many(
            instrument_ids.iter().map(|instrument_id| {
                us_stock_minute_bar_ingest_progress::ActiveModel {
                    instrument_id: Set(instrument_id.clone()),
                    completed_through: Set(completed_through.fixed_offset()),
                }
            }),
        )
        .on_conflict(
            OnConflict::column(us_stock_minute_bar_ingest_progress::Column::InstrumentId)
                .value(
                    us_stock_minute_bar_ingest_progress::Column::CompletedThrough,
                    Expr::cust(
                        "GREATEST(us_stock_minute_bar_ingest_progress.completed_through, EXCLUDED.completed_through)",
                    ),
                )
                .to_owned(),
        )
        .exec_without_returning(&self.db)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }

    async fn compress_bars_older_than_policy(
        &self,
        now: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError> {
        #[derive(FromQueryResult)]
        struct CompressionCutoff {
            cutoff: DateTime<FixedOffset>,
        }

        let policy_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"SELECT $1 - (config ->> 'compress_after')::interval AS cutoff
                FROM timescaledb_information.jobs
                WHERE hypertable_schema = current_schema()
                  AND hypertable_name = 'bars'
                  AND proc_name = 'policy_compression'"#,
                [now.fixed_offset().into()],
            ))
            .await
            .map_err(repository_error)?
            .ok_or_else(|| {
                BarsRepositoryError::Database(PersistenceError::Database(
                    "bars compression policy is not configured".to_owned(),
                ))
            })?;
        let cutoff = CompressionCutoff::from_query_result(&policy_row, "")
            .map_err(repository_error)?
            .cutoff;

        self.db
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"SELECT compress_chunk(
                    format('%I.%I', chunk_schema, chunk_name)::regclass,
                    if_not_compressed => TRUE
                )
                FROM timescaledb_information.chunks
                WHERE hypertable_schema = current_schema()
                  AND hypertable_name = 'bars'
                  AND range_end < $1
                  AND is_compressed = FALSE"#,
                [cutoff.into()],
            ))
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<HashSet<NaiveDate>, BarsRepositoryError> {
        jquants_daily_bars_ingested_date::Entity::find()
            .filter(jquants_daily_bars_ingested_date::Column::Date.gte(from))
            .all(&self.db)
            .await
            .map(|rows| rows.into_iter().map(|row| row.date).collect())
            .map_err(repository_error)
    }

    async fn ensure_instruments_exist(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_ids: &HashSet<String>,
    ) -> Result<(), BarsRepositoryError> {
        if instrument_ids.is_empty() {
            return Ok(());
        }

        let transaction = transaction_ref(transaction)?;
        let models = instrument_ids
            .iter()
            .map(|instrument_id| instruments::ActiveModel {
                id: Set(instrument_id.clone()),
                name: Set(instrument_id.clone()),
                market: Set("TSE".to_string()),
                sector: Set(None),
            });
        instruments::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(instruments::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map(|_| ())
            .map_err(repository_error)
    }

    async fn upsert_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        bar_queries::upsert_bars(transaction, bars)
            .await
            .map_err(repository_error)
    }

    async fn mark_ingested(
        &self,
        transaction: &UnitOfWorkTransaction,
        date: NaiveDate,
    ) -> Result<(), BarsRepositoryError> {
        let transaction = transaction_ref(transaction)?;
        jquants_daily_bars_ingested_date::Entity::insert(
            jquants_daily_bars_ingested_date::ActiveModel { date: Set(date) },
        )
        .on_conflict(
            OnConflict::column(jquants_daily_bars_ingested_date::Column::Date)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map(|_| ())
        .map_err(repository_error)
    }
}

fn to_postgres_query(query: BarsQuery) -> PostgresBarsQuery {
    PostgresBarsQuery {
        instrument_id: query.instrument_id,
        timeframe: query.timeframe,
        from: query.from,
        to: query.to,
    }
}

fn transaction_ref(
    transaction: &UnitOfWorkTransaction,
) -> Result<&sea_orm::DatabaseTransaction, BarsRepositoryError> {
    postgres_transaction_ref(transaction).ok_or(BarsRepositoryError::InvalidTransaction)
}

fn repository_error(error: sea_orm::DbErr) -> BarsRepositoryError {
    BarsRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use chrono::{
        DateTime, Duration as ChronoDuration, FixedOffset, NaiveDate, NaiveTime, TimeZone,
        Timelike, Utc,
    };
    use core_application::bars::{
        BarsQuery, BarsRepository, BarsUseCases, DailyBarAdjustmentFactor,
        SharedUsStockBarsWriteLock, UsStockBarTarget, UsStockBarsIngestUseCases,
        UsStockBarsWriteLock, UsStockBarsWriteLockGuard, UsStockMinuteBarsDownsampleStats,
        us_stock_hourly_history_cutoff,
    };
    use core_application::persistence::PersistenceError;
    use core_application::unit_of_work::{SharedUnitOfWork, UnitOfWork};
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::{ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, Statement};
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::entities::{
        bars, group_axis, instruments, note, note_ref, stock, stock_group, stock_group_member,
    };
    use crate::{DatabaseHandle, PostgresUnitOfWork};

    use super::{PostgresBarsQuery, PostgresBarsRepository, bar_queries};

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    async fn read_bar_coverage(
        db: &impl ConnectionTrait,
        instrument_id: &str,
    ) -> (DateTime<FixedOffset>, DateTime<FixedOffset>) {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT earliest_timestamp, latest_timestamp FROM bars_timestamp_coverage \
                 WHERE instrument_id = $1 AND timeframe = '1m'",
                [instrument_id.into()],
            ))
            .await
            .expect("find minute coverage")
            .expect("minute coverage row");
        (
            row.try_get::<DateTime<FixedOffset>>("", "earliest_timestamp")
                .expect("decode earliest minute"),
            row.try_get::<DateTime<FixedOffset>>("", "latest_timestamp")
                .expect("decode latest minute"),
        )
    }

    struct TestWriteLock;

    struct TestWriteLockGuard;

    impl UsStockBarsWriteLockGuard for TestWriteLockGuard {}

    #[async_trait::async_trait]
    impl UsStockBarsWriteLock for TestWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: Duration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            Ok(Some(Box::new(TestWriteLockGuard)))
        }
    }

    #[derive(Debug, FromQueryResult, PartialEq, Eq)]
    struct BarChunkCompressionStatus {
        old_chunk_compressed: bool,
        recent_chunk_compressed: bool,
    }

    #[derive(Debug, FromQueryResult)]
    struct CompressedChunkStatus {
        is_compressed: bool,
    }

    #[backend_test_macros::database_test]
    async fn writes_bars_and_ingest_dates_through_the_same_transaction(db: DatabaseHandle) {
        let instrument_id = "SAMPLE-DB-ALPHA";
        let date = NaiveDate::from_ymd_opt(2099, 7, 3).expect("fixture date");
        let bar = Bar {
            instrument_id: instrument_id.to_string(),
            timeframe: Timeframe::Daily,
            timestamp: Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight")),
            open: Decimal::new(140, 0),
            high: Decimal::new(150, 0),
            low: Decimal::new(130, 0),
            close: Decimal::new(145, 0),
            volume: 1000,
            adjustment_factor: Decimal::ONE,
        };
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");

        repository
            .ensure_instruments_exist(&transaction, &HashSet::from([instrument_id.into()]))
            .await
            .expect("ensure instrument");
        repository
            .upsert_bars(&transaction, vec![bar.clone()])
            .await
            .expect("upsert bar");
        repository
            .mark_ingested(&transaction, date)
            .await
            .expect("mark date");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        let bars = repository
            .find_bars(BarsQuery {
                instrument_id: instrument_id.to_string(),
                timeframe: "1d".into(),
                from: None,
                to: None,
            })
            .await
            .expect("find bars");
        let ingested_dates = repository
            .find_ingested_dates(date)
            .await
            .expect("find ingested dates");

        assert_eq!((bars, ingested_dates), (vec![bar], HashSet::from([date])),);
    }

    #[backend_test_macros::database_test]
    async fn upsert_bars_merges_coverage_across_transactions(db: DatabaseHandle) {
        let instrument_id = format!("US:DEMO-COVERAGE-{}", uuid::Uuid::new_v4().simple());
        let timestamp = |day| {
            Utc.with_ymd_and_hms(2099, 7, day, 0, 0, 0)
                .single()
                .expect("fixture timestamp")
        };
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());

        let first_transaction = unit_of_work.begin().await.expect("begin first transaction");
        repository
            .ensure_instruments_exist(&first_transaction, &HashSet::from([instrument_id.clone()]))
            .await
            .expect("ensure instrument");
        repository
            .upsert_bars(
                &first_transaction,
                vec![
                    test_bar(&instrument_id, Timeframe::Minute, timestamp(2)),
                    test_bar(&instrument_id, Timeframe::Minute, timestamp(3)),
                ],
            )
            .await
            .expect("upsert first bars");
        unit_of_work
            .commit(first_transaction)
            .await
            .expect("commit first transaction");
        let initial_coverage = read_bar_coverage(&db, &instrument_id).await;

        let second_transaction = unit_of_work
            .begin()
            .await
            .expect("begin second transaction");
        repository
            .upsert_bars(
                &second_transaction,
                vec![test_bar(&instrument_id, Timeframe::Minute, timestamp(1))],
            )
            .await
            .expect("extend earliest coverage");
        unit_of_work
            .commit(second_transaction)
            .await
            .expect("commit second transaction");
        let earliest_coverage = read_bar_coverage(&db, &instrument_id).await;

        let third_transaction = unit_of_work.begin().await.expect("begin third transaction");
        repository
            .upsert_bars(
                &third_transaction,
                vec![test_bar(&instrument_id, Timeframe::Minute, timestamp(4))],
            )
            .await
            .expect("extend latest coverage");
        unit_of_work
            .commit(third_transaction)
            .await
            .expect("commit third transaction");

        let expanded_coverage = read_bar_coverage(&db, &instrument_id).await;

        assert_eq!(
            (initial_coverage, earliest_coverage, expanded_coverage),
            (
                (timestamp(2).fixed_offset(), timestamp(3).fixed_offset()),
                (timestamp(1).fixed_offset(), timestamp(3).fixed_offset()),
                (timestamp(1).fixed_offset(), timestamp(4).fixed_offset()),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn finds_daily_adjustment_factors_from_the_requested_date(db: DatabaseHandle) {
        let suffix = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        let instrument_id = suffix[..4].to_string();
        let other_instrument_id = suffix[4..8].to_string();
        for id in [&instrument_id, &other_instrument_id] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.to_string()),
                name: Set(id.to_string()),
                market: Set("TSE".to_string()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
        }

        let bar = |instrument_id: &str, date: NaiveDate, factor: Decimal| bars::ActiveModel {
            instrument_id: Set(instrument_id.to_string()),
            timeframe: Set("1d".to_string()),
            timestamp: Set(date.and_time(NaiveTime::MIN).and_utc().fixed_offset()),
            open: Set(Decimal::new(100, 0)),
            high: Set(Decimal::new(100, 0)),
            low: Set(Decimal::new(100, 0)),
            close: Set(Decimal::new(100, 0)),
            volume: Set(1),
            adjustment_factor: Set(factor),
        };
        bars::Entity::insert_many(vec![
            bar(&instrument_id, date(2099, 1, 1), Decimal::new(5, 1)),
            bar(&instrument_id, date(2099, 1, 2), Decimal::ONE),
            bar(&instrument_id, date(2099, 1, 5), Decimal::new(5, 1)),
            bar(&instrument_id, date(2099, 2, 3), Decimal::new(25, 2)),
            bar(&other_instrument_id, date(2099, 1, 7), Decimal::new(2, 1)),
        ])
        .exec_without_returning(&db)
        .await
        .expect("insert daily bars");

        let repository = PostgresBarsRepository::new(db);
        let factors = repository
            .find_daily_adjustment_factors_from(&instrument_id, date(2099, 1, 2))
            .await
            .expect("find adjustment factors");

        assert_eq!(
            factors,
            vec![
                DailyBarAdjustmentFactor {
                    date: date(2099, 1, 5),
                    factor: Decimal::new(5, 1),
                },
                DailyBarAdjustmentFactor {
                    date: date(2099, 2, 3),
                    factor: Decimal::new(25, 2),
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn finds_grouped_and_note_referenced_us_stocks_with_latest_bars(db: DatabaseHandle) {
        let suffix = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        let group_stock_id = format!("US:QZ-GROUP-{suffix}");
        let note_stock_id = format!("US:QZ-NOTE-{suffix}");
        let unlinked_stock_id = format!("US:QZ-UNLINKED-{suffix}");
        let wrong_kind_stock_id = format!("US:QZ-OTHER-{suffix}");
        let non_us_group_stock_id = format!("KR:QZ-GROUP-{suffix}");

        for (id, instrument_market, stock_market) in [
            (&group_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&note_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&unlinked_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&wrong_kind_stock_id, "US", "SYNTHETIC-US-EXCHANGE"),
            (&non_us_group_stock_id, "OTHER", "SYNTHETIC-OTHER-EXCHANGE"),
        ] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(instrument_market.to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
            stock::Entity::insert(stock::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(Some(stock_market.to_owned())),
                created_at: NotSet,
                updated_at: NotSet,
                product_category: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert stock");
        }

        let axis_id = uuid::Uuid::new_v4();
        group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(axis_id),
            key: Set(format!("synthetic-axis-{suffix}")),
            name: Set("Synthetic Axis".to_owned()),
            description: Set("Synthetic axis for a database test".to_owned()),
            derive_from: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert group axis");
        let group_id = uuid::Uuid::new_v4();
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(group_id),
            axis_id: Set(axis_id),
            key: Set(format!("synthetic-group-{suffix}")),
            name: Set("Synthetic Group".to_owned()),
            description: Set(None),
            code: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert stock group");
        for stock_id in [&group_stock_id, &non_us_group_stock_id] {
            stock_group_member::Entity::insert(stock_group_member::ActiveModel {
                stock_id: Set(stock_id.clone()),
                group_id: Set(group_id),
                created_at: NotSet,
            })
            .exec_without_returning(&db)
            .await
            .expect("insert group membership");
        }

        let note_id = uuid::Uuid::new_v4();
        note::Entity::insert(note::ActiveModel {
            id: Set(note_id),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert note");
        for (ref_kind, ref_id) in [
            ("stock", group_stock_id.as_str()),
            ("stock", note_stock_id.as_str()),
            ("indicator", wrong_kind_stock_id.as_str()),
        ] {
            note_ref::Entity::insert(note_ref::ActiveModel {
                note_id: Set(note_id),
                ref_kind: Set(ref_kind.to_owned()),
                ref_id: Set(ref_id.to_owned()),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert note reference");
        }

        let daily_latest = Utc
            .with_ymd_and_hms(2040, 1, 2, 0, 0, 0)
            .single()
            .expect("latest daily timestamp");
        let note_daily_latest = Utc
            .with_ymd_and_hms(2040, 1, 3, 0, 0, 0)
            .single()
            .expect("note daily timestamp");
        let minute_latest = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 4, 0)
            .single()
            .expect("latest minute timestamp");
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert_bars(
                &transaction,
                vec![
                    test_bar(
                        &group_stock_id,
                        Timeframe::Daily,
                        Utc.with_ymd_and_hms(2040, 1, 1, 0, 0, 0)
                            .single()
                            .expect("prior daily timestamp"),
                    ),
                    test_bar(&group_stock_id, Timeframe::Daily, daily_latest),
                    test_bar(&note_stock_id, Timeframe::Daily, note_daily_latest),
                    test_bar(
                        &group_stock_id,
                        Timeframe::Minute,
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 3, 0)
                            .single()
                            .expect("prior minute timestamp"),
                    ),
                    test_bar(&group_stock_id, Timeframe::Minute, minute_latest),
                    test_bar(
                        &group_stock_id,
                        Timeframe::Hourly,
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 2, 0)
                            .single()
                            .expect("prior hourly timestamp"),
                    ),
                    test_bar(
                        &group_stock_id,
                        Timeframe::Hourly,
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 5, 0)
                            .single()
                            .expect("latest hourly timestamp"),
                    ),
                ],
            )
            .await
            .expect("insert bars");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        let initial_hourly_progress = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 10, 0)
            .single()
            .expect("hourly progress timestamp");
        let later_hourly_progress = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 20, 0)
            .single()
            .expect("later hourly progress timestamp");
        let older_hourly_progress = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 0, 0)
            .single()
            .expect("older hourly progress timestamp");
        let initial_minute_progress = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 6, 0)
            .single()
            .expect("minute progress timestamp");
        let later_minute_progress = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 12, 0)
            .single()
            .expect("later minute progress timestamp");
        let older_minute_progress = Utc
            .with_ymd_and_hms(2040, 1, 2, 15, 0, 0)
            .single()
            .expect("older minute progress timestamp");
        repository
            .mark_us_stock_hourly_bar_history_completed(
                std::slice::from_ref(&group_stock_id),
                initial_hourly_progress,
            )
            .await
            .expect("mark initial hourly progress");
        repository
            .mark_us_stock_hourly_bar_history_completed(
                std::slice::from_ref(&group_stock_id),
                later_hourly_progress,
            )
            .await
            .expect("mark later hourly progress");
        repository
            .mark_us_stock_hourly_bar_history_completed(
                std::slice::from_ref(&group_stock_id),
                older_hourly_progress,
            )
            .await
            .expect("ignore older hourly progress");
        repository
            .mark_us_stock_minute_bar_history_completed(
                std::slice::from_ref(&note_stock_id),
                initial_minute_progress,
            )
            .await
            .expect("mark initial minute progress");
        repository
            .mark_us_stock_minute_bar_history_completed(
                std::slice::from_ref(&note_stock_id),
                later_minute_progress,
            )
            .await
            .expect("mark later minute progress");
        repository
            .mark_us_stock_minute_bar_history_completed(
                std::slice::from_ref(&note_stock_id),
                older_minute_progress,
            )
            .await
            .expect("ignore older minute progress");
        repository
            .mark_us_stock_minute_bar_history_completed(&[], later_minute_progress)
            .await
            .expect("empty minute progress update succeeds");

        let actual = repository
            .find_us_stock_minute_bar_targets()
            .await
            .expect("find US stock bar targets");

        assert_eq!(
            actual,
            vec![
                UsStockBarTarget {
                    instrument_id: group_stock_id,
                    latest_daily_bar: Some(daily_latest),
                    latest_minute_bar: Some(minute_latest),
                    latest_hourly_bar: Some(
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 5, 0)
                            .single()
                            .expect("latest hourly timestamp"),
                    ),
                    minute_history_completed_through: None,
                    hourly_history_completed_through: Some(later_hourly_progress),
                    earliest_daily_bar: Some(
                        Utc.with_ymd_and_hms(2040, 1, 1, 0, 0, 0)
                            .single()
                            .expect("earliest daily timestamp"),
                    ),
                    earliest_minute_bar: Some(
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 3, 0)
                            .single()
                            .expect("earliest minute timestamp"),
                    ),
                    earliest_hourly_bar: Some(
                        Utc.with_ymd_and_hms(2040, 1, 2, 15, 2, 0)
                            .single()
                            .expect("earliest hourly timestamp"),
                    ),
                },
                UsStockBarTarget {
                    instrument_id: note_stock_id,
                    latest_daily_bar: Some(note_daily_latest),
                    latest_minute_bar: None,
                    latest_hourly_bar: None,
                    minute_history_completed_through: Some(later_minute_progress),
                    hourly_history_completed_through: None,
                    earliest_daily_bar: Some(note_daily_latest),
                    earliest_minute_bar: None,
                    earliest_hourly_bar: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn finds_unreferenced_us_stocks_on_nasdaq_and_nyse_for_daily_ingestion(
        db: DatabaseHandle,
    ) {
        let suffix = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        let nasdaq_id = format!("US:QZ-DAILY-NASDAQ-{suffix}");
        let nyse_id = format!("US:QZ-DAILY-NYSE-{suffix}");
        let otc_id = format!("US:QZ-DAILY-OTC-{suffix}");
        let referenced_otc_id = format!("US:QZ-DAILY-OTC-REFERENCED-{suffix}");
        let non_us_id = format!("KR:QZ-DAILY-NASDAQ-{suffix}");

        for (id, instrument_market, exchange) in [
            (&nasdaq_id, "US", "Nasdaq"),
            (&nyse_id, "US", "NYSE"),
            (&otc_id, "US", "OTC"),
            (&referenced_otc_id, "US", "OTC"),
            (&non_us_id, "OTHER", "Nasdaq"),
        ] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(instrument_market.to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
            stock::Entity::insert(stock::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(Some(exchange.to_owned())),
                created_at: NotSet,
                updated_at: NotSet,
                product_category: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert stock");
        }

        let note_id = uuid::Uuid::new_v4();
        note::Entity::insert(note::ActiveModel {
            id: Set(note_id),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert note");
        note_ref::Entity::insert(note_ref::ActiveModel {
            note_id: Set(note_id),
            ref_kind: Set("stock".to_owned()),
            ref_id: Set(referenced_otc_id.clone()),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert note reference");

        let actual = PostgresBarsRepository::new(db)
            .find_us_stock_daily_bar_targets()
            .await
            .expect("find US stock daily bar targets");

        assert_eq!(
            actual,
            vec![
                UsStockBarTarget {
                    instrument_id: nasdaq_id,
                    latest_daily_bar: None,
                    latest_minute_bar: None,
                    latest_hourly_bar: None,
                    minute_history_completed_through: None,
                    hourly_history_completed_through: None,
                    earliest_daily_bar: None,
                    earliest_minute_bar: None,
                    earliest_hourly_bar: None,
                },
                UsStockBarTarget {
                    instrument_id: nyse_id,
                    latest_daily_bar: None,
                    latest_minute_bar: None,
                    latest_hourly_bar: None,
                    minute_history_completed_through: None,
                    hourly_history_completed_through: None,
                    earliest_daily_bar: None,
                    earliest_minute_bar: None,
                    earliest_hourly_bar: None,
                },
                UsStockBarTarget {
                    instrument_id: referenced_otc_id,
                    latest_daily_bar: None,
                    latest_minute_bar: None,
                    latest_hourly_bar: None,
                    minute_history_completed_through: None,
                    hourly_history_completed_through: None,
                    earliest_daily_bar: None,
                    earliest_minute_bar: None,
                    earliest_hourly_bar: None,
                },
            ],
        );
    }

    #[backend_test_macros::database_test]
    async fn finds_earliest_us_minute_bar_before_cutoff_independent_of_coverage(
        db: DatabaseHandle,
    ) {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let earliest_us_id = format!("US:DEMO-EARLY-{suffix}");
        let later_us_id = format!("US:DEMO-LATER-{suffix}");
        let no_earlier_us_id = format!("US:DEMO-NO-EARLIER-{suffix}");
        let foreign_id = format!("KR:DEMO-MINUTE-{suffix}");
        for (id, market) in [
            (&earliest_us_id, "US"),
            (&later_us_id, "US"),
            (&no_earlier_us_id, "US"),
            (&foreign_id, "OTHER"),
        ] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(market.to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
        }

        let timestamp = |hour, minute| {
            Utc.with_ymd_and_hms(2040, 1, 2, hour, minute, 0)
                .single()
                .expect("fixture timestamp")
        };
        bar_queries::upsert_bars(
            &db,
            vec![
                test_bar(&earliest_us_id, Timeframe::Minute, timestamp(8, 0)),
                test_bar(&earliest_us_id, Timeframe::Minute, timestamp(9, 0)),
                test_bar(&earliest_us_id, Timeframe::Minute, timestamp(9, 1)),
                test_bar(&earliest_us_id, Timeframe::Hourly, timestamp(6, 0)),
                test_bar(&later_us_id, Timeframe::Minute, timestamp(8, 30)),
                test_bar(&later_us_id, Timeframe::Minute, timestamp(9, 0)),
                test_bar(&later_us_id, Timeframe::Minute, timestamp(9, 1)),
                test_bar(&no_earlier_us_id, Timeframe::Minute, timestamp(9, 0)),
                test_bar(&no_earlier_us_id, Timeframe::Minute, timestamp(9, 1)),
                test_bar(&foreign_id, Timeframe::Minute, timestamp(7, 0)),
            ],
        )
        .await
        .expect("insert synthetic bars");
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "DELETE FROM bars_timestamp_coverage WHERE instrument_id = $1 AND timeframe = '1m'",
            [earliest_us_id.into()],
        ))
        .await
        .expect("remove minute coverage");
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE bars_timestamp_coverage SET earliest_timestamp = $2 \
             WHERE instrument_id = $1 AND timeframe = '1m'",
            [later_us_id.into(), timestamp(7, 30).fixed_offset().into()],
        ))
        .await
        .expect("make minute coverage stale");

        let repository = PostgresBarsRepository::new(db);
        let before_cutoff = repository
            .find_earliest_us_stock_minute_bar_before(timestamp(9, 0))
            .await
            .expect("find earliest US minute bar before cutoff");
        let at_cutoff = repository
            .find_earliest_us_stock_minute_bar_before(timestamp(8, 0))
            .await
            .expect("find US minute bars before an equal cutoff");

        assert_eq!((before_cutoff, at_cutoff), (Some(timestamp(8, 0)), None),);
    }

    #[backend_test_macros::database_test]
    async fn finds_no_minute_bar_when_no_us_instrument_exists(db: DatabaseHandle) {
        let instrument_id = format!(
            "KR:DEMO-EMPTY-{suffix}",
            suffix = uuid::Uuid::new_v4().simple()
        );
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(instrument_id.clone()),
            name: Set(instrument_id.clone()),
            market: Set("OTHER".to_owned()),
            sector: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert foreign instrument");
        let cutoff = Utc
            .with_ymd_and_hms(2040, 1, 2, 9, 0, 0)
            .single()
            .expect("fixture cutoff");
        bar_queries::upsert_bars(
            &db,
            vec![test_bar(
                &instrument_id,
                Timeframe::Minute,
                cutoff - ChronoDuration::minutes(1),
            )],
        )
        .await
        .expect("insert foreign minute bar");

        let earliest = PostgresBarsRepository::new(db)
            .find_earliest_us_stock_minute_bar_before(cutoff)
            .await
            .expect("find earliest US minute bar");

        assert_eq!(earliest, None);
    }

    #[backend_test_macros::database_test]
    async fn downsampling_preserves_api_hourly_bars_and_deletes_only_saved_us_minutes(
        db: DatabaseHandle,
    ) {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let us_instrument_id = format!("US:DEMO-DOWNSAMPLE-{suffix}");
        let empty_us_instrument_id = format!("US:DEMO-EMPTY-{suffix}");
        let partial_bucket_instrument_id = format!("US:DEMO-PARTIAL-BUCKET-{suffix}");
        let fully_deleted_instrument_id = format!("US:DEMO-FULLY-DELETED-{suffix}");
        let foreign_instrument_id = format!("JP-DEMO-DOWNSAMPLE-{suffix}");
        for (id, market) in [
            (&us_instrument_id, "US"),
            (&empty_us_instrument_id, "US"),
            (&partial_bucket_instrument_id, "US"),
            (&fully_deleted_instrument_id, "US"),
            (&foreign_instrument_id, "OTHER"),
        ] {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(market.to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
        }

        let timestamp = |hour, minute| {
            Utc.with_ymd_and_hms(2040, 1, 2, hour, minute, 0)
                .single()
                .expect("fixture timestamp")
        };
        let bar = |instrument_id: &str, timeframe, at, open, high, low, close, volume| Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp: at,
            open: Decimal::new(open, 0),
            high: Decimal::new(high, 0),
            low: Decimal::new(low, 0),
            close: Decimal::new(close, 0),
            volume,
            adjustment_factor: Decimal::ONE,
        };
        let api_hourly_bar = bar(
            &us_instrument_id,
            Timeframe::Hourly,
            timestamp(8, 0),
            100,
            110,
            90,
            105,
            77,
        );
        let expected_latest_minute = bar(
            &us_instrument_id,
            Timeframe::Minute,
            timestamp(9, 0),
            200,
            210,
            190,
            205,
            11,
        );
        let expected_latest_hourly = bar(
            &us_instrument_id,
            Timeframe::Hourly,
            timestamp(9, 0),
            200,
            210,
            190,
            205,
            11,
        );
        let existing_hourly_bar = bar(
            &us_instrument_id,
            Timeframe::Hourly,
            timestamp(6, 0),
            9,
            25,
            7,
            19,
            500,
        );
        let inserted_hourly_bar = bar(
            &us_instrument_id,
            Timeframe::Hourly,
            timestamp(7, 0),
            30,
            45,
            28,
            38,
            700,
        );
        let foreign_minute = bar(
            &foreign_instrument_id,
            Timeframe::Minute,
            timestamp(7, 0),
            300,
            310,
            290,
            305,
            13,
        );
        let partial_bucket_minute_to_delete = bar(
            &partial_bucket_instrument_id,
            Timeframe::Minute,
            timestamp(6, 15),
            50,
            60,
            45,
            55,
            20,
        );
        let partial_bucket_minute = bar(
            &partial_bucket_instrument_id,
            Timeframe::Minute,
            timestamp(8, 15),
            60,
            70,
            55,
            65,
            25,
        );
        let fully_deleted_minute = bar(
            &fully_deleted_instrument_id,
            Timeframe::Minute,
            timestamp(6, 15),
            70,
            80,
            65,
            75,
            30,
        );
        let cutoff = timestamp(9, 0);
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        bar_queries::upsert_bars(
            &db,
            vec![
                bar(
                    &us_instrument_id,
                    Timeframe::Minute,
                    timestamp(6, 30),
                    10,
                    15,
                    8,
                    12,
                    100,
                ),
                bar(
                    &us_instrument_id,
                    Timeframe::Minute,
                    timestamp(6, 31),
                    12,
                    20,
                    9,
                    18,
                    200,
                ),
                bar(
                    &us_instrument_id,
                    Timeframe::Minute,
                    timestamp(7, 0),
                    30,
                    35,
                    28,
                    32,
                    400,
                ),
                bar(
                    &us_instrument_id,
                    Timeframe::Minute,
                    timestamp(7, 1),
                    32,
                    45,
                    30,
                    38,
                    300,
                ),
                bar(
                    &us_instrument_id,
                    Timeframe::Minute,
                    timestamp(8, 0),
                    1,
                    5,
                    0,
                    3,
                    2,
                ),
                bar(
                    &us_instrument_id,
                    Timeframe::Minute,
                    timestamp(8, 1),
                    3,
                    6,
                    2,
                    4,
                    3,
                ),
                existing_hourly_bar.clone(),
                api_hourly_bar.clone(),
                expected_latest_minute.clone(),
                foreign_minute.clone(),
                partial_bucket_minute_to_delete,
                partial_bucket_minute.clone(),
                fully_deleted_minute,
            ],
        )
        .await
        .expect("insert test bars");
        let earliest = repository
            .find_earliest_us_stock_minute_bar_before(cutoff)
            .await
            .expect("find earliest old US minute bar");
        let instrument_ids = repository
            .find_us_stock_instrument_ids()
            .await
            .expect("find US stock instruments");
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let stats = repository
            .downsample_us_stock_minute_bar_range(
                &transaction,
                &us_instrument_id,
                timestamp(6, 30),
                timestamp(10, 0),
                cutoff,
            )
            .await
            .expect("downsample old US minute bars");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit downsample transaction");

        let partial_bucket_transaction = unit_of_work
            .begin()
            .await
            .expect("begin partial bucket downsample");
        let partial_bucket_stats = repository
            .downsample_us_stock_minute_bar_range(
                &partial_bucket_transaction,
                &partial_bucket_instrument_id,
                timestamp(6, 0),
                timestamp(9, 0),
                timestamp(8, 30),
            )
            .await
            .expect("downsample before partial cutoff hour");
        unit_of_work
            .commit(partial_bucket_transaction)
            .await
            .expect("commit partial bucket downsample");

        let fully_deleted_transaction = unit_of_work
            .begin()
            .await
            .expect("begin fully deleted downsample");
        let fully_deleted_stats = repository
            .downsample_us_stock_minute_bar_range(
                &fully_deleted_transaction,
                &fully_deleted_instrument_id,
                timestamp(6, 0),
                timestamp(9, 0),
                timestamp(8, 30),
            )
            .await
            .expect("downsample all complete minute buckets");
        unit_of_work
            .commit(fully_deleted_transaction)
            .await
            .expect("commit fully deleted downsample");

        let hourly_bars = crate::repositories::bars::find_intraday_bars_by_instruments(
            &db,
            std::slice::from_ref(&us_instrument_id),
            Timeframe::Hourly,
            Some(cutoff.fixed_offset()),
            None,
            None,
        )
        .await
        .expect("find hourly bars");
        let us_minutes = repository
            .find_bars(BarsQuery {
                instrument_id: us_instrument_id.clone(),
                timeframe: "1m".to_owned(),
                from: None,
                to: None,
            })
            .await
            .expect("find remaining US minute bars");
        let foreign_minutes = bar_queries::find_bars(
            &db,
            PostgresBarsQuery {
                instrument_id: foreign_instrument_id,
                timeframe: "1m".to_owned(),
                from: None,
                to: None,
            },
        )
        .await
        .expect("find remaining foreign minute bars")
        .into_iter()
        .map(Into::into)
        .collect::<Vec<Bar>>();
        let coverage_row = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT earliest_timestamp FROM bars_timestamp_coverage \
                 WHERE instrument_id = $1 AND timeframe = '1m'",
                [us_instrument_id.clone().into()],
            ))
            .await
            .expect("find remaining minute coverage")
            .expect("minute coverage row");
        let earliest_minute_after: DateTime<FixedOffset> = coverage_row
            .try_get("", "earliest_timestamp")
            .expect("decode earliest minute");
        let partial_bucket_minutes = repository
            .find_bars(BarsQuery {
                instrument_id: partial_bucket_instrument_id.clone(),
                timeframe: "1m".to_owned(),
                from: None,
                to: None,
            })
            .await
            .expect("find partial bucket minute bars");
        let partial_bucket_coverage = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT earliest_timestamp FROM bars_timestamp_coverage \
                 WHERE instrument_id = $1 AND timeframe = '1m'",
                [partial_bucket_instrument_id.clone().into()],
            ))
            .await
            .expect("find partial bucket minute coverage")
            .expect("partial bucket minute coverage row")
            .try_get::<DateTime<FixedOffset>>("", "earliest_timestamp")
            .expect("decode partial bucket earliest minute");
        let fully_deleted_minutes = repository
            .find_bars(BarsQuery {
                instrument_id: fully_deleted_instrument_id.clone(),
                timeframe: "1m".to_owned(),
                from: None,
                to: None,
            })
            .await
            .expect("find fully deleted minute bars");
        let fully_deleted_coverage_count = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT count(*)::bigint AS row_count FROM bars_timestamp_coverage \
                 WHERE instrument_id = $1 AND timeframe = '1m'",
                [fully_deleted_instrument_id.clone().into()],
            ))
            .await
            .expect("count coverage rows after all minutes are deleted")
            .expect("coverage count row")
            .try_get::<i64>("", "row_count")
            .expect("decode coverage count");

        assert_eq!(
            (
                (earliest, earliest_minute_after, instrument_ids),
                (stats, hourly_bars, us_minutes, foreign_minutes),
                (
                    partial_bucket_stats,
                    partial_bucket_minutes,
                    partial_bucket_coverage,
                ),
                (
                    fully_deleted_stats,
                    fully_deleted_minutes,
                    fully_deleted_coverage_count,
                ),
            ),
            (
                (
                    Some(timestamp(6, 15)),
                    timestamp(9, 0).fixed_offset(),
                    vec![
                        us_instrument_id.clone(),
                        empty_us_instrument_id,
                        fully_deleted_instrument_id.clone(),
                        partial_bucket_instrument_id.clone(),
                    ],
                ),
                (
                    UsStockMinuteBarsDownsampleStats {
                        hourly_bars_inserted: 1,
                        minute_bars_deleted: 6,
                    },
                    vec![
                        existing_hourly_bar,
                        inserted_hourly_bar,
                        api_hourly_bar,
                        expected_latest_hourly,
                    ],
                    vec![expected_latest_minute],
                    vec![foreign_minute],
                ),
                (
                    UsStockMinuteBarsDownsampleStats {
                        hourly_bars_inserted: 1,
                        minute_bars_deleted: 1,
                    },
                    vec![partial_bucket_minute.clone()],
                    timestamp(8, 15).fixed_offset(),
                ),
                (
                    UsStockMinuteBarsDownsampleStats {
                        hourly_bars_inserted: 1,
                        minute_bars_deleted: 1,
                    },
                    Vec::new(),
                    0,
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn downsampling_compressed_chunks_stays_under_the_per_transaction_limit(
        db: DatabaseHandle,
    ) {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let instrument_ids = [
            format!("US:DEMO-LIMIT-ALPHA-{suffix}"),
            format!("US:DEMO-LIMIT-BETA-{suffix}"),
        ];
        for instrument_id in &instrument_ids {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(instrument_id.clone()),
                name: Set(instrument_id.clone()),
                market: Set("US".to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
        }

        let now = Utc::now();
        let cutoff = us_stock_hourly_history_cutoff(now);
        let minute_timestamp = cutoff - ChronoDuration::days(14);
        let repository = PostgresBarsRepository::new(db.clone());
        bar_queries::upsert_bars(
            &db,
            instrument_ids
                .iter()
                .map(|instrument_id| test_bar(instrument_id, Timeframe::Minute, minute_timestamp))
                .collect(),
        )
        .await
        .expect("insert old minute bars");
        repository
            .compress_bars_older_than_policy(now)
            .await
            .expect("compress old bars");

        let chunk_status = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                indoc::indoc! { r#"SELECT is_compressed
                FROM timescaledb_information.chunks
                WHERE hypertable_schema = current_schema()
                  AND hypertable_name = 'bars'
                  AND range_start <= $1
                  AND range_end > $1"# },
                [minute_timestamp.fixed_offset().into()],
            ))
            .await
            .expect("read chunk compression status")
            .expect("compressed chunk status row");
        let chunk_status = CompressedChunkStatus::from_query_result(&chunk_status, "")
            .expect("decode chunk compression status");
        // 外側の rollback transaction 内で実行するため、このテストでは commit 境界を検証しない。
        // 銘柄ごとの commit 境界は fake のテストで固定する。
        db.execute_unprepared(
            "SET LOCAL timescaledb.max_tuples_decompressed_per_dml_transaction = 1",
        )
        .await
        .expect("limit tuples decompressed per transaction");

        let unit_of_work: SharedUnitOfWork =
            std::sync::Arc::new(PostgresUnitOfWork::new(db.clone()));
        let repository: core_application::bars::SharedBarsRepository =
            std::sync::Arc::new(repository);
        let write_lock: SharedUsStockBarsWriteLock = Arc::new(TestWriteLock);
        let result = UsStockBarsIngestUseCases::new(
            BarsUseCases::new(unit_of_work, repository.clone()),
            write_lock.clone(),
            write_lock,
        )
        .downsample_old_us_stock_minute_bars(now)
        .await
        .map_err(|_| ());
        let mut remaining_minutes = Vec::new();
        for instrument_id in &instrument_ids {
            remaining_minutes.push(
                repository
                    .find_bars(BarsQuery {
                        instrument_id: instrument_id.clone(),
                        timeframe: "1m".to_owned(),
                        from: None,
                        to: None,
                    })
                    .await
                    .expect("read remaining minute bars")
                    .len(),
            );
        }

        assert_eq!(
            (chunk_status.is_compressed, result, remaining_minutes),
            (
                true,
                Ok(UsStockMinuteBarsDownsampleStats {
                    hourly_bars_inserted: 2,
                    minute_bars_deleted: 2,
                }),
                vec![0, 0],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn unfinished_hour_remains_visible_between_downsample_runs(db: DatabaseHandle) {
        let instrument_id = format!("US:DEMO-PARTIAL-{}", uuid::Uuid::new_v4().simple());
        instruments::Entity::insert(instruments::ActiveModel {
            id: Set(instrument_id.clone()),
            name: Set(instrument_id.clone()),
            market: Set("US".to_owned()),
            sector: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert instrument");

        let timestamp = |hour, minute| {
            Utc.with_ymd_and_hms(2040, 1, 2, hour, minute, 0)
                .single()
                .expect("fixture timestamp")
        };
        let bar = |timeframe, at, open, high, low, close, volume| Bar {
            instrument_id: instrument_id.clone(),
            timeframe,
            timestamp: at,
            open: Decimal::new(open, 0),
            high: Decimal::new(high, 0),
            low: Decimal::new(low, 0),
            close: Decimal::new(close, 0),
            volume,
            adjustment_factor: Decimal::ONE,
        };
        let minute_bars = vec![
            bar(Timeframe::Minute, timestamp(10, 0), 10, 12, 9, 11, 100),
            bar(Timeframe::Minute, timestamp(10, 1), 11, 15, 10, 14, 200),
        ];
        bar_queries::upsert_bars(&db, minute_bars)
            .await
            .expect("insert minute bars");

        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let cutoff = timestamp(10, 30);
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let stats = repository
            .downsample_us_stock_minute_bar_range(
                &transaction,
                &instrument_id,
                timestamp(10, 0),
                timestamp(11, 0),
                cutoff,
            )
            .await
            .expect("downsample partial hour");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit downsample transaction");
        let hourly_bars = bar_queries::find_intraday_bars_by_instruments(
            &db,
            std::slice::from_ref(&instrument_id),
            Timeframe::Hourly,
            Some(timestamp(11, 0).fixed_offset()),
            None,
            None,
        )
        .await
        .expect("find hourly bars");

        assert_eq!(
            (stats, hourly_bars),
            (
                UsStockMinuteBarsDownsampleStats::default(),
                vec![bar(Timeframe::Hourly, timestamp(10, 0), 10, 15, 9, 14, 300)],
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn minute_targets_match_daily_targets_for_all_eligible_stocks_and_references(
        db: DatabaseHandle,
    ) {
        let suffix = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        let candidate_ids = (0..102)
            .map(|index| format!("US:DEMO-{suffix}-{index:03}"))
            .collect::<Vec<_>>();
        let group_stock_id = format!("US:ZZ-DEMO-GROUP-{suffix}");
        let note_stock_id = format!("US:ZZ-DEMO-NOTE-{suffix}");
        let unreferenced_otc_id = format!("US:DEMO-{suffix}-000A-OTC");
        let non_us_nasdaq_id = format!("CA:DEMO-{suffix}-NASDAQ");

        for (id, instrument_market, exchange) in candidate_ids
            .iter()
            .enumerate()
            .map(|(index, id)| (id, "US", if index == 50 { "NYSE" } else { "Nasdaq" }))
            .chain([
                (&group_stock_id, "US", "OTC"),
                (&note_stock_id, "US", "OTC"),
                (&unreferenced_otc_id, "US", "OTC"),
                (&non_us_nasdaq_id, "OTHER", "Nasdaq"),
            ])
        {
            instruments::Entity::insert(instruments::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(instrument_market.to_owned()),
                sector: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert instrument");
            stock::Entity::insert(stock::ActiveModel {
                id: Set(id.clone()),
                name: Set(id.clone()),
                market: Set(Some(exchange.to_owned())),
                created_at: NotSet,
                updated_at: NotSet,
                product_category: Set(None),
            })
            .exec_without_returning(&db)
            .await
            .expect("insert stock");
        }

        let axis_id = uuid::Uuid::new_v4();
        group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(axis_id),
            key: Set(format!("synthetic-axis-{suffix}")),
            name: Set("Synthetic Axis".to_owned()),
            description: Set("Synthetic axis for a database test".to_owned()),
            derive_from: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert group axis");
        let group_id = uuid::Uuid::new_v4();
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(group_id),
            axis_id: Set(axis_id),
            key: Set(format!("synthetic-group-{suffix}")),
            name: Set("Synthetic Group".to_owned()),
            description: Set(None),
            code: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert stock group");
        stock_group_member::Entity::insert(stock_group_member::ActiveModel {
            stock_id: Set(group_stock_id.clone()),
            group_id: Set(group_id),
            created_at: NotSet,
        })
        .exec_without_returning(&db)
        .await
        .expect("insert group membership");

        let note_id = uuid::Uuid::new_v4();
        note::Entity::insert(note::ActiveModel {
            id: Set(note_id),
            kind: Set(None),
            trigger: Set(None),
            trigger_label: Set(None),
            created_at: NotSet,
            updated_at: NotSet,
            execution_id: Set(None),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert note");
        note_ref::Entity::insert(note_ref::ActiveModel {
            note_id: Set(note_id),
            ref_kind: Set("stock".to_owned()),
            ref_id: Set(note_stock_id.clone()),
        })
        .exec_without_returning(&db)
        .await
        .expect("insert note reference");

        let repository = PostgresBarsRepository::new(db);
        let actual_minute = repository
            .find_us_stock_minute_bar_targets()
            .await
            .expect("find US stock minute bar targets");
        let actual_daily = repository
            .find_us_stock_daily_bar_targets()
            .await
            .expect("find US stock daily bar targets");
        let mut expected_ids = candidate_ids;
        expected_ids.extend([group_stock_id, note_stock_id]);
        expected_ids.sort();
        let expected = expected_ids
            .into_iter()
            .map(|instrument_id| UsStockBarTarget {
                instrument_id,
                latest_daily_bar: None,
                latest_minute_bar: None,
                latest_hourly_bar: None,
                minute_history_completed_through: None,
                hourly_history_completed_through: None,
                earliest_daily_bar: None,
                earliest_minute_bar: None,
                earliest_hourly_bar: None,
            })
            .collect::<Vec<_>>();

        assert_eq!((actual_minute, actual_daily), (expected.clone(), expected));
    }

    #[backend_test_macros::database_test]
    async fn compresses_uncompressed_bar_chunks_older_than_the_policy_delay(db: DatabaseHandle) {
        let instrument_id = format!(
            "COMPRESSION-{}",
            uuid::Uuid::new_v4().simple().to_string().to_uppercase()
        );
        let now = Utc::now();
        let old_bar_time = now - ChronoDuration::days(90);
        let recent_bar_time = now - ChronoDuration::days(2);
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .ensure_instruments_exist(&transaction, &HashSet::from([instrument_id.clone()]))
            .await
            .expect("ensure instrument");
        repository
            .upsert_bars(
                &transaction,
                vec![
                    test_bar(&instrument_id, Timeframe::Minute, old_bar_time),
                    test_bar(&instrument_id, Timeframe::Minute, recent_bar_time),
                ],
            )
            .await
            .expect("insert minute bars");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");

        repository
            .compress_bars_older_than_policy(now)
            .await
            .expect("compress old bars");

        let status = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"SELECT
                    (
                        SELECT is_compressed
                        FROM timescaledb_information.chunks
                        WHERE hypertable_schema = current_schema()
                          AND hypertable_name = 'bars'
                          AND range_start <= $1
                          AND range_end > $1
                    ) AS old_chunk_compressed,
                    (
                        SELECT is_compressed
                        FROM timescaledb_information.chunks
                        WHERE hypertable_schema = current_schema()
                          AND hypertable_name = 'bars'
                          AND range_start <= $2
                          AND range_end > $2
                    ) AS recent_chunk_compressed"#,
                [
                    old_bar_time.fixed_offset().into(),
                    recent_bar_time.fixed_offset().into(),
                ],
            ))
            .await
            .expect("read chunk compression status")
            .expect("chunk compression status row");
        let status = BarChunkCompressionStatus::from_query_result(&status, "")
            .expect("decode chunk compression status");

        assert_eq!(
            status,
            BarChunkCompressionStatus {
                old_chunk_compressed: true,
                recent_chunk_compressed: false,
            },
        );
    }

    #[backend_test_macros::database_test]
    async fn upserts_minute_bar_in_a_compressed_chunk(db: DatabaseHandle) {
        let instrument_id = format!(
            "COMPRESSED-UPSERT-{}",
            uuid::Uuid::new_v4().simple().to_string().to_uppercase()
        );
        let now = Utc::now();
        let old_bar_time = now - ChronoDuration::days(90);
        let old_bar_time = old_bar_time
            .with_nanosecond(old_bar_time.timestamp_subsec_micros() * 1_000)
            .expect("valid nanosecond");
        let repository = PostgresBarsRepository::new(db.clone());
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .ensure_instruments_exist(&transaction, &HashSet::from([instrument_id.clone()]))
            .await
            .expect("ensure instrument");
        repository
            .upsert_bars(
                &transaction,
                vec![test_bar(&instrument_id, Timeframe::Minute, old_bar_time)],
            )
            .await
            .expect("insert minute bar");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit initial minute bar");

        repository
            .compress_bars_older_than_policy(now)
            .await
            .expect("compress old bar");

        let mut updated_bar = test_bar(&instrument_id, Timeframe::Minute, old_bar_time);
        updated_bar.open = Decimal::new(249, 1);
        updated_bar.high = Decimal::new(251, 1);
        updated_bar.low = Decimal::new(248, 1);
        updated_bar.close = Decimal::new(250, 1);
        updated_bar.volume = 40;
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        repository
            .upsert_bars(&transaction, vec![updated_bar.clone()])
            .await
            .expect("upsert minute bar in compressed chunk");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit compressed chunk upsert");

        let chunk_status = db
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"SELECT is_compressed
                FROM timescaledb_information.chunks
                WHERE hypertable_schema = current_schema()
                  AND hypertable_name = 'bars'
                  AND range_start <= $1
                  AND range_end > $1"#,
                [old_bar_time.fixed_offset().into()],
            ))
            .await
            .expect("read compressed chunk status")
            .expect("compressed chunk status row");
        let chunk_status = CompressedChunkStatus::from_query_result(&chunk_status, "")
            .expect("decode compressed chunk status");

        let bars = bar_queries::find_bars(
            &db,
            PostgresBarsQuery {
                instrument_id,
                timeframe: "1m".to_owned(),
                from: Some(old_bar_time.fixed_offset()),
                to: Some(old_bar_time.fixed_offset()),
            },
        )
        .await
        .expect("read bars from compressed chunk")
        .into_iter()
        .map(Into::into)
        .collect::<Vec<Bar>>();

        assert_eq!(
            (chunk_status.is_compressed, bars),
            (true, vec![updated_bar])
        );
    }

    fn test_bar(instrument_id: &str, timeframe: Timeframe, timestamp: DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: Decimal::new(101, 1),
            high: Decimal::new(111, 1),
            low: Decimal::new(91, 1),
            close: Decimal::new(105, 1),
            volume: 25,
            adjustment_factor: Decimal::ONE,
        }
    }
}
