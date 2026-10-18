use sea_orm_migration::prelude::*;
use sea_orm::{DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum BarsTimestampCoverage {
    Table,
    InstrumentId,
    Timeframe,
    EarliestTimestamp,
    LatestTimestamp,
}

#[derive(DeriveIden)]
enum Instruments {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // backfill の範囲ごとに commit し、長い transaction を作らない。
    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .create_table(
                Table::create()
                    .table(BarsTimestampCoverage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BarsTimestampCoverage::InstrumentId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BarsTimestampCoverage::Timeframe)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BarsTimestampCoverage::EarliestTimestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BarsTimestampCoverage::LatestTimestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(BarsTimestampCoverage::InstrumentId)
                            .col(BarsTimestampCoverage::Timeframe),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                BarsTimestampCoverage::Table,
                                BarsTimestampCoverage::InstrumentId,
                            )
                            .to(Instruments::Table, Instruments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(
                        Expr::col(BarsTimestampCoverage::Timeframe).is_in(["1m", "1h", "1d"]),
                    )
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "COMMENT ON TABLE bars_timestamp_coverage IS '銘柄と時間足ごとの最古・最新のバー時刻を保持する。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN bars_timestamp_coverage.instrument_id IS 'バーの銘柄コード。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN bars_timestamp_coverage.timeframe IS 'バーの時間足。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN bars_timestamp_coverage.earliest_timestamp IS '保存されている最古のバー時刻。'",
        )
        .await?;
        db.execute_unprepared(
            "COMMENT ON COLUMN bars_timestamp_coverage.latest_timestamp IS '保存されている最新のバー時刻。'",
        )
        .await?;

        let ranges = db
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT range_start::text AS range_start, range_end::text AS range_end \
                 FROM timescaledb_information.chunks \
                 WHERE hypertable_schema = current_schema() AND hypertable_name = 'bars' \
                 ORDER BY range_start",
            ))
            .await?;

        // 各 statement を独立して commit し、途中失敗時も次回実行で再開できるようにする。
        for range_batch in ranges.chunks(8) {
            let start: String = range_batch
                .first()
                .ok_or_else(|| DbErr::Custom("bars chunk range batch is empty".to_owned()))?
                .try_get("", "range_start")?;
            let end: String = range_batch
                .last()
                .ok_or_else(|| DbErr::Custom("bars chunk range batch is empty".to_owned()))?
                .try_get("", "range_end")?;

            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO bars_timestamp_coverage \
                    (instrument_id, timeframe, earliest_timestamp, latest_timestamp) \
                 SELECT instrument_id, timeframe, MIN(timestamp), MAX(timestamp) \
                 FROM bars \
                 WHERE timestamp >= $1::timestamptz AND timestamp < $2::timestamptz \
                 GROUP BY instrument_id, timeframe \
                 ON CONFLICT (instrument_id, timeframe) DO UPDATE SET \
                    earliest_timestamp = LEAST(bars_timestamp_coverage.earliest_timestamp, EXCLUDED.earliest_timestamp), \
                    latest_timestamp = GREATEST(bars_timestamp_coverage.latest_timestamp, EXCLUDED.latest_timestamp)",
                [start.into(), end.into()],
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BarsTimestampCoverage::Table)
                    .to_owned(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use sea_orm::{
        ConnectOptions, ConnectionTrait, Database, DbBackend, Statement, TransactionTrait,
    };
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    fn benchmark_setting(name: &str, default: usize) -> usize {
        match std::env::var(name) {
            Ok(value) => value
                .parse()
                .expect("benchmark settings must be unsigned integers"),
            Err(std::env::VarError::NotPresent) | Err(std::env::VarError::NotUnicode(_)) => default,
        }
    }

    #[tokio::test]
    #[ignore = "manual benchmark on a dedicated disposable TimescaleDB database"]
    async fn benchmark_backfill_with_compressed_synthetic_bars() {
        const DEFAULT_INSTRUMENTS: usize = 7_700;
        const DEFAULT_WEEKS: usize = 8;

        let instruments = benchmark_setting("BARS_COVERAGE_BENCH_INSTRUMENTS", DEFAULT_INSTRUMENTS);
        let weeks = benchmark_setting("BARS_COVERAGE_BENCH_WEEKS", DEFAULT_WEEKS);
        let minute_weeks = benchmark_setting(
            "BARS_COVERAGE_BENCH_MINUTE_WEEKS",
            weeks.min(DEFAULT_WEEKS),
        );
        let minute_step_minutes = benchmark_setting("BARS_COVERAGE_BENCH_MINUTE_STEP_MINUTES", 10);
        let compressed_chunk_target = benchmark_setting(
            "BARS_COVERAGE_BENCH_COMPRESSED_CHUNKS",
            weeks.saturating_sub(2),
        );
        assert!(instruments > 0 && weeks > 0);
        assert!(minute_weeks <= weeks);
        assert!(minute_step_minutes > 0 && 390 % minute_step_minutes == 0);
        assert!(compressed_chunk_target <= weeks);
        let instruments_i32 = i32::try_from(instruments).expect("instrument count fits in integer");
        let weeks_i32 = i32::try_from(weeks).expect("week count fits in integer");
        let minute_weeks_i32 = i32::try_from(minute_weeks).expect("minute week count fits in integer");
        let minute_step_i32 = i32::try_from(minute_step_minutes).expect("minute step fits in integer");
        let samples_per_day = 390 / minute_step_minutes;
        let samples_per_day_i32 =
            i32::try_from(samples_per_day).expect("minute sample count fits in integer");

        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let mut options = ConnectOptions::new(database_url);
        options.max_connections(1);
        let database = Database::connect(options)
            .await
            .expect("connect to benchmark database");
        let database_name: String = database
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT current_database() AS database_name",
            ))
            .await
            .expect("read benchmark database name")
            .expect("database name row")
            .try_get("", "database_name")
            .expect("decode benchmark database name");
        assert!(
            database_name.starts_with("t_rader_backend_bars_coverage_bench_"),
            "refusing to write benchmark data to non-benchmark database: {database_name}"
        );

        database
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .await
            .expect("enable TimescaleDB");

        let schema = format!("bars_coverage_bench_{}", std::process::id());
        database
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create isolated schema");
        database
            .execute_unprepared(&format!("SET search_path TO {schema}, public"))
            .await
            .expect("set isolated schema");
        database
            .execute_unprepared(
                "CREATE TABLE instruments (
                    id text PRIMARY KEY,
                    benchmark_id integer NOT NULL
                )",
            )
            .await
            .expect("create instruments table");
        database
            .execute_unprepared(
                "CREATE TABLE bars (
                    instrument_id text NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
                    timeframe text NOT NULL,
                    timestamp timestamptz NOT NULL,
                    open numeric NOT NULL,
                    high numeric NOT NULL,
                    low numeric NOT NULL,
                    close numeric NOT NULL,
                    volume bigint NOT NULL,
                    PRIMARY KEY (instrument_id, timeframe, timestamp)
                )",
            )
            .await
            .expect("create bars table");
        database
            .execute_unprepared(
                "SELECT create_hypertable('bars', by_range('timestamp', INTERVAL '7 days'))",
            )
            .await
            .expect("create seven-day bars hypertable");
        database
            .execute_unprepared(
                "ALTER TABLE bars SET (
                    timescaledb.compress = true,
                    timescaledb.compress_segmentby = 'instrument_id,timeframe'
                )",
            )
            .await
            .expect("configure bars compression");

        let generation_started = Instant::now();
        let instrument_insert = database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO instruments (id, benchmark_id)
                 SELECT 'US:BENCH-' || lpad(instrument_id::text, 5, '0'), instrument_id
                 FROM generate_series(1, $1::integer) AS instrument_id",
                [instruments_i32.into()],
            ))
            .await
            .expect("insert synthetic instruments");
        assert_eq!(instrument_insert.rows_affected(), instruments as u64);

        let mut inserted_rows = 0_u64;
        for week in 0..weeks_i32 {
            let result = database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "WITH week AS (
                    SELECT TIMESTAMPTZ '2040-01-05 00:00:00+00' + ($1::integer * INTERVAL '7 days') AS start_at,
                           $1::integer AS week_number
                 ), minute_params AS (
                    SELECT $2::integer AS samples_per_day,
                           $3::integer AS first_minute_week,
                           $4::integer AS step_minutes
                 )
                 INSERT INTO bars
                    (instrument_id, timeframe, timestamp, open, high, low, close, volume)
                 SELECT i.id, '1d', week.start_at + day.day * INTERVAL '1 day',
                        price.value, price.value + 0.02, price.value - 0.02,
                        price.value + 0.01, 10000 + i.benchmark_id * 7 + day.day
                 FROM instruments AS i
                 CROSS JOIN week
                 CROSS JOIN generate_series(0, 4) AS day(day)
                 CROSS JOIN LATERAL (
                    SELECT 10::numeric + i.benchmark_id::numeric / 100
                           + (week.week_number * 5 + day.day)::numeric / 10000 AS value
                 ) AS price
                 UNION ALL
                 SELECT i.id, '1h', week.start_at + day.day * INTERVAL '1 day'
                                      + (13 + hour.hour) * INTERVAL '1 hour',
                        price.value, price.value + 0.02, price.value - 0.02,
                        price.value + 0.01, 20000 + i.benchmark_id * 7 + day.day * 7 + hour.hour
                 FROM instruments AS i
                 CROSS JOIN week
                 CROSS JOIN generate_series(0, 4) AS day(day)
                 CROSS JOIN generate_series(0, 6) AS hour(hour)
                 CROSS JOIN LATERAL (
                    SELECT 10::numeric + i.benchmark_id::numeric / 100
                           + (week.week_number * 35 + day.day * 7 + hour.hour)::numeric / 10000 AS value
                 ) AS price
                 UNION ALL
                 SELECT i.id, '1m', week.start_at + day.day * INTERVAL '1 day'
                                      + INTERVAL '9 hours 30 minutes'
                                      + sample.minute * params.step_minutes * INTERVAL '1 minute',
                        price.value, price.value + 0.02, price.value - 0.02,
                        price.value + 0.01, 30000 + i.benchmark_id * 7
                            + day.day * params.samples_per_day + sample.minute
                 FROM instruments AS i
                 CROSS JOIN week
                 CROSS JOIN minute_params AS params
                 CROSS JOIN generate_series(0, 4) AS day(day)
                 CROSS JOIN LATERAL generate_series(0, params.samples_per_day - 1) AS sample(minute)
                 CROSS JOIN LATERAL (
                    SELECT 10::numeric + i.benchmark_id::numeric / 100
                           + (week.week_number * params.samples_per_day * 5
                              + day.day * params.samples_per_day + sample.minute)::numeric / 10000 AS value
                 ) AS price
                 WHERE week.week_number >= params.first_minute_week",
                    [
                        week.into(),
                        samples_per_day_i32.into(),
                        (weeks_i32 - minute_weeks_i32).into(),
                        minute_step_i32.into(),
                    ],
                ))
                .await
                .expect("insert synthetic bars");
            inserted_rows += result.rows_affected();
            let progress_interval = (weeks_i32 / 20).max(1);
            if (week + 1) % progress_interval == 0 || week + 1 == weeks_i32 {
                eprintln!("generated synthetic bars for {} of {weeks} weeks", week + 1);
            }
        }
        let generation_duration = generation_started.elapsed();
        let expected_rows = instruments as u64
            * (weeks as u64 * 40 + minute_weeks as u64 * 5 * samples_per_day as u64);
        assert_eq!(inserted_rows, expected_rows);

        let chunk_rows = database
            .query_all_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT chunk_schema, chunk_name
                 FROM timescaledb_information.chunks
                 WHERE hypertable_schema = current_schema() AND hypertable_name = 'bars'
                 ORDER BY range_start",
            ))
            .await
            .expect("list bars chunks");
        assert_eq!(chunk_rows.len(), weeks);

        let compression_started = Instant::now();
        for chunk in chunk_rows.iter().take(compressed_chunk_target) {
            let chunk_schema: String = chunk.try_get("", "chunk_schema").expect("chunk schema");
            let chunk_name: String = chunk.try_get("", "chunk_name").expect("chunk name");
            let relation = format!("\"{chunk_schema}\".\"{chunk_name}\"");
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT compress_chunk($1::regclass)",
                    [relation.into()],
                ))
                .await
                .expect("compress bars chunk");
        }
        let compression_duration = compression_started.elapsed();
        let compressed_bytes: i64 = database
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT coalesce(sum(after_compression_total_bytes), 0)::bigint AS bytes
                 FROM chunk_compression_stats('bars'::regclass)",
            ))
            .await
            .expect("read compressed chunk sizes")
            .expect("compressed size row")
            .try_get("", "bytes")
            .expect("decode compressed size");

        let migration_started = Instant::now();
        Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("run bars timestamp coverage migration");
        let migration_duration = migration_started.elapsed();

        let coverage_rows: i64 = database
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT count(*) AS row_count FROM bars_timestamp_coverage",
            ))
            .await
            .expect("read coverage row count")
            .expect("coverage count row")
            .try_get("", "row_count")
            .expect("decode coverage row count");
        let compressed_chunk_count: i64 = database
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT count(*) FILTER (WHERE is_compressed) AS compressed_count
                 FROM timescaledb_information.chunks
                 WHERE hypertable_schema = current_schema() AND hypertable_name = 'bars'",
            ))
            .await
            .expect("count compressed chunks")
            .expect("compressed chunk count row")
            .try_get("", "compressed_count")
            .expect("decode compressed chunk count");

        eprintln!(
            "benchmark results: instruments={instruments}, weeks={weeks}, bars={inserted_rows}, \
             chunks={}/{}, compressed_chunks={compressed_chunk_count}, compressed_bytes={compressed_bytes}, \
             coverage_rows={coverage_rows}, generation={generation_duration:?}, \
             compression={compression_duration:?}, migration={migration_duration:?}",
            chunk_rows.len(),
            weeks,
        );
        let expected_coverage_rows = instruments as i64 * if minute_weeks > 0 { 3 } else { 2 };
        assert_eq!(coverage_rows, expected_coverage_rows);
        assert_eq!(compressed_chunk_count, compressed_chunk_target as i64);
    }

    #[tokio::test]
    async fn migration_backfills_coverage_across_chunk_batches() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        database
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .await
            .expect("enable TimescaleDB");

        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("bars_coverage_test_{}", std::process::id());
        transaction
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .expect("create isolated schema");
        transaction
            .execute_unprepared(&format!("SET LOCAL search_path TO {schema}, public"))
            .await
            .expect("set isolated schema");
        transaction
            .execute_unprepared("CREATE TABLE instruments (id text PRIMARY KEY)")
            .await
            .expect("create instruments table");
        transaction
            .execute_unprepared(
                "CREATE TABLE bars (
                    instrument_id text NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
                    timeframe text NOT NULL,
                    timestamp timestamptz NOT NULL,
                    open numeric NOT NULL,
                    high numeric NOT NULL,
                    low numeric NOT NULL,
                    close numeric NOT NULL,
                    volume bigint NOT NULL,
                    PRIMARY KEY (instrument_id, timeframe, timestamp)
                )",
            )
            .await
            .expect("create bars table");
        transaction
            .execute_unprepared(
                "SELECT create_hypertable('bars', by_range('timestamp', INTERVAL '7 days'))",
            )
            .await
            .expect("create bars hypertable");
        transaction
            .execute_unprepared(
                "INSERT INTO instruments (id) VALUES ('US:QZ-COVERAGE-MIGRATION')",
            )
            .await
            .expect("insert instrument");
        transaction
            .execute_unprepared(
                "INSERT INTO bars (instrument_id, timeframe, timestamp, open, high, low, close, volume)
                 SELECT 'US:QZ-COVERAGE-MIGRATION', '1d',
                    '2040-01-01 00:00:00+00'::timestamptz + (week * INTERVAL '7 days'),
                    10, 10, 10, 10, 1
                 FROM generate_series(0, 523) AS weeks(week)",
            )
            .await
            .expect("insert daily bars");

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("run bars timestamp coverage migration");

        let chunk_count: i64 = transaction
            .query_one_raw(sea_orm::Statement::from_string(
                DbBackend::Postgres,
                "SELECT count(*) FROM timescaledb_information.chunks \
                 WHERE hypertable_schema = current_schema() AND hypertable_name = 'bars'",
            ))
            .await
            .expect("count bars chunks")
            .expect("bars chunks row")
            .try_get("", "count")
            .expect("decode bars chunk count");
        let expected_rows = transaction
            .query_all_raw(sea_orm::Statement::from_string(
                DbBackend::Postgres,
                "SELECT timeframe,
                    to_char(MIN(timestamp) AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') AS earliest_timestamp,
                    to_char(MAX(timestamp) AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') AS latest_timestamp
                 FROM bars
                 GROUP BY timeframe
                 ORDER BY timeframe",
            ))
            .await
            .expect("read source bar bounds");
        let expected = expected_rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get::<String>("", "timeframe")?,
                    row.try_get::<String>("", "earliest_timestamp")?,
                    row.try_get::<String>("", "latest_timestamp")?,
                ))
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()
            .expect("decode source bar bounds");
        let rows = transaction
            .query_all_raw(sea_orm::Statement::from_string(
                DbBackend::Postgres,
                "SELECT timeframe,
                    to_char(earliest_timestamp AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') AS earliest_timestamp,
                    to_char(latest_timestamp AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS') AS latest_timestamp
                 FROM bars_timestamp_coverage
                 ORDER BY timeframe",
            ))
            .await
            .expect("read bar timestamp coverage");
        let actual = rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get::<String>("", "timeframe")?,
                    row.try_get::<String>("", "earliest_timestamp")?,
                    row.try_get::<String>("", "latest_timestamp")?,
                ))
            })
            .collect::<Result<Vec<_>, sea_orm::DbErr>>()
            .expect("decode bar timestamp coverage");

        assert_eq!((chunk_count, actual), (524, expected));
    }
}
