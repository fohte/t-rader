use sea_orm_migration::prelude::*;
use sea_orm::{DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    // backfill の範囲ごとに commit し、長い transaction を作らない。
    fn use_transaction(&self) -> Option<bool> {
        Some(false)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS bars_timestamp_coverage (
                instrument_id text NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
                timeframe text NOT NULL CHECK (timeframe IN ('1m', '1h', '1d')),
                earliest_timestamp timestamptz NOT NULL,
                latest_timestamp timestamptz NOT NULL,
                PRIMARY KEY (instrument_id, timeframe)
            )",
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
            .drop_table(Table::drop().table(Alias::new("bars_timestamp_coverage")).to_owned())
            .await
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

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
