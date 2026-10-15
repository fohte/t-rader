use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Bars {
    Table,
}

#[derive(DeriveIden)]
enum MinuteBars {
    Table,
    InstrumentId,
    Timestamp,
    Open,
    High,
    Low,
    Close,
    Volume,
}

#[derive(DeriveIden)]
enum Instruments {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        manager
            .alter_table(
                Table::alter()
                    .table(Bars::Table)
                    .drop_constraint(Alias::new("bars_timeframe_check"))
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "ALTER TABLE bars \
             ADD CONSTRAINT bars_timeframe_check \
             CHECK (timeframe IN ('1m', '1h', '1d'))",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO bars (instrument_id, timeframe, timestamp, open, high, low, close, volume) \
             SELECT instrument_id, '1m', timestamp, open, high, low, close, volume \
             FROM minute_bars",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE bars SET (timescaledb.compress = true, timescaledb.compress_segmentby = 'instrument_id,timeframe')",
        )
        .await?;
        db.execute_unprepared("SELECT add_compression_policy('bars', INTERVAL '7 days')")
            .await?;
        db.execute_unprepared("SELECT remove_compression_policy('minute_bars')")
            .await?;
        manager
            .drop_table(Table::drop().table(MinuteBars::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DO $$
             BEGIN
                 IF EXISTS (SELECT 1 FROM bars WHERE timeframe = '1h') THEN
                     RAISE EXCEPTION 'cannot restore minute_bars while 1h bars exist';
                 END IF;
             END;
             $$",
        )
        .await?;
        db.execute_unprepared("SELECT remove_compression_policy('bars')")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MinuteBars::Table)
                    .col(ColumnDef::new(MinuteBars::InstrumentId).string().not_null())
                    .col(
                        ColumnDef::new(MinuteBars::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MinuteBars::Open).decimal().not_null())
                    .col(ColumnDef::new(MinuteBars::High).decimal().not_null())
                    .col(ColumnDef::new(MinuteBars::Low).decimal().not_null())
                    .col(ColumnDef::new(MinuteBars::Close).decimal().not_null())
                    .col(ColumnDef::new(MinuteBars::Volume).big_integer().not_null())
                    .primary_key(
                        Index::create()
                            .col(MinuteBars::InstrumentId)
                            .col(MinuteBars::Timestamp),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MinuteBars::Table, MinuteBars::InstrumentId)
                            .to(Instruments::Table, Instruments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        db.execute_unprepared(
            "SELECT create_hypertable('minute_bars', by_range('timestamp', INTERVAL '1 day'))",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE minute_bars SET (timescaledb.compress = true, timescaledb.compress_segmentby = 'instrument_id')",
        )
        .await?;
        db.execute_unprepared(
            "INSERT INTO minute_bars (instrument_id, timestamp, open, high, low, close, volume) \
             SELECT instrument_id, timestamp, open, high, low, close, volume \
             FROM bars WHERE timeframe = '1m'",
        )
        .await?;
        db.execute_unprepared("DELETE FROM bars WHERE timeframe = '1m'")
            .await?;
        db.execute_unprepared("ALTER TABLE bars DROP CONSTRAINT bars_timeframe_check")
            .await?;
        db.execute_unprepared(
            "ALTER TABLE bars ADD CONSTRAINT bars_timeframe_check CHECK (timeframe IN ('1d'))",
        )
        .await?;
        db.execute_unprepared("SELECT add_compression_policy('minute_bars', INTERVAL '7 days')")
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "テスト準備の失敗時に詳細を表示するため")]

    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn migration_merges_and_restores_minute_bars() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        database
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .await
            .expect("enable TimescaleDB");

        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("bars_merge_test_{}", std::process::id());
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
                    adjustment_factor numeric NOT NULL DEFAULT 1,
                    PRIMARY KEY (instrument_id, timeframe, timestamp),
                    CONSTRAINT bars_timeframe_check CHECK (timeframe IN ('1d'))
                )",
            )
            .await
            .expect("create bars table");
        transaction
            .execute_unprepared("SELECT create_hypertable('bars', by_range('timestamp'))")
            .await
            .expect("create bars hypertable");
        transaction
            .execute_unprepared("INSERT INTO instruments (id) VALUES ('US:DEMO-A')")
            .await
            .expect("insert instrument");
        transaction
            .execute_unprepared(
                "INSERT INTO bars (instrument_id, timeframe, timestamp, open, high, low, close, volume) \
                 VALUES ('US:DEMO-A', '1d', '2040-01-01T00:00:00Z', 100, 110, 90, 105, 10)",
            )
            .await
            .expect("insert daily bar");
        transaction
            .execute_unprepared(
                "CREATE TABLE minute_bars (
                    instrument_id text NOT NULL REFERENCES instruments(id) ON DELETE CASCADE,
                    timestamp timestamptz NOT NULL,
                    open numeric NOT NULL,
                    high numeric NOT NULL,
                    low numeric NOT NULL,
                    close numeric NOT NULL,
                    volume bigint NOT NULL,
                    PRIMARY KEY (instrument_id, timestamp)
                )",
            )
            .await
            .expect("create minute_bars table");
        transaction
            .execute_unprepared(
                "SELECT create_hypertable('minute_bars', by_range('timestamp', INTERVAL '1 day'))",
            )
            .await
            .expect("create minute_bars hypertable");
        transaction
            .execute_unprepared(
                "ALTER TABLE minute_bars SET (timescaledb.compress = true, timescaledb.compress_segmentby = 'instrument_id')",
            )
            .await
            .expect("enable minute_bars compression");
        transaction
            .execute_unprepared(
                "SELECT add_compression_policy('minute_bars', INTERVAL '7 days')",
            )
            .await
            .expect("add minute_bars compression policy");
        transaction
            .execute_unprepared(
                "INSERT INTO minute_bars (instrument_id, timestamp, open, high, low, close, volume) \
                 VALUES ('US:DEMO-A', '2040-01-02T15:04:00Z', 200, 210, 190, 205, 20)",
            )
            .await
            .expect("insert minute bar");

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("merge minute bars");
        transaction
            .execute_unprepared(
                "INSERT INTO bars (instrument_id, timeframe, timestamp, open, high, low, close, volume) \
                 VALUES ('US:DEMO-A', '1h', '2040-01-02T15:00:00Z', 200, 220, 180, 210, 100)",
            )
            .await
            .expect("insert hourly bar");
        transaction
            .execute_unprepared("DELETE FROM bars WHERE timeframe = '1h'")
            .await
            .expect("remove hourly test row");

        let settings = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                indoc::indoc! {
                    "SELECT
                        compression_enabled,
                        (
                            SELECT time_interval::text
                            FROM timescaledb_information.dimensions
                            WHERE hypertable_schema = $1
                              AND hypertable_name = 'bars'
                              AND column_name = 'timestamp'
                        ) AS chunk_interval,
                        (
                            SELECT string_agg(attname::text, ',' ORDER BY segmentby_column_index)
                            FROM timescaledb_information.compression_settings
                            WHERE hypertable_schema = $1
                              AND hypertable_name = 'bars'
                              AND segmentby_column_index > 0
                        ) AS segmentby_columns,
                        (
                            SELECT config ->> 'compress_after'
                            FROM timescaledb_information.jobs
                            WHERE hypertable_schema = $1
                              AND hypertable_name = 'bars'
                              AND proc_name = 'policy_compression'
                        ) AS compress_after,
                        (
                            SELECT concat_ws(
                                '|', instrument_id, timeframe,
                                to_char(timestamp AT TIME ZONE 'UTC', 'YYYYMMDDHH24MISS'),
                                open::text, high::text, low::text, close::text, volume::text
                            )
                            FROM bars WHERE timeframe = '1m'
                        ) AS migrated_minute_bar,
                        to_regclass('minute_bars') IS NULL AS minute_table_removed
                    FROM timescaledb_information.hypertables
                    WHERE hypertable_schema = $1
                      AND hypertable_name = 'bars'"
                },
                [schema.clone().into()],
            ))
            .await
            .expect("read merged hypertable settings")
            .expect("merged hypertable settings row");

        assert_eq!(
            (
                settings
                    .try_get::<bool>("", "compression_enabled")
                    .expect("read compression status"),
                settings
                    .try_get::<Option<String>>("", "chunk_interval")
                    .expect("read chunk interval"),
                settings
                    .try_get::<Option<String>>("", "segmentby_columns")
                    .expect("read segmentby columns"),
                settings
                    .try_get::<Option<String>>("", "compress_after")
                    .expect("read compression policy"),
                settings
                    .try_get::<Option<String>>("", "migrated_minute_bar")
                    .expect("read migrated minute bar"),
                settings
                    .try_get::<bool>("", "minute_table_removed")
                    .expect("read minute_bars removal"),
            ),
            (
                true,
                Some("7 days".into()),
                Some("instrument_id,timeframe".into()),
                Some("7 days".into()),
                Some("US:DEMO-A|1m|20400102150400|200|210|190|205|20".into()),
                true,
            ),
        );

        Migration
            .down(&SchemaManager::new(&transaction))
            .await
            .expect("restore minute bars");
        let restored = transaction
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                indoc::indoc! {
                    "SELECT
                        (SELECT string_agg(timeframe, ',' ORDER BY timeframe) FROM bars) AS bar_timeframes,
                        (
                            SELECT time_interval::text
                            FROM timescaledb_information.dimensions
                            WHERE hypertable_schema = current_schema()
                              AND hypertable_name = 'bars'
                              AND column_name = 'timestamp'
                        ) AS bars_chunk_interval,
                        (SELECT count(*) FROM minute_bars) AS minute_count,
                        (SELECT close::integer FROM minute_bars) AS minute_close,
                        EXISTS (
                            SELECT 1 FROM timescaledb_information.jobs
                            WHERE hypertable_schema = current_schema()
                              AND hypertable_name = 'minute_bars'
                              AND proc_name = 'policy_compression'
                        ) AS minute_policy_restored,
                        NOT EXISTS (
                            SELECT 1 FROM timescaledb_information.jobs
                            WHERE hypertable_schema = current_schema()
                              AND hypertable_name = 'bars'
                              AND proc_name = 'policy_compression'
                        ) AS bars_policy_removed"
                },
            ))
            .await
            .expect("read restored schema")
            .expect("restored schema row");

        assert_eq!(
            (
                restored
                    .try_get::<Option<String>>("", "bar_timeframes")
                    .expect("read bar timeframes"),
                restored
                    .try_get::<Option<String>>("", "bars_chunk_interval")
                    .expect("read bars chunk interval"),
                restored
                    .try_get::<i64>("", "minute_count")
                    .expect("read restored minute count"),
                restored
                    .try_get::<i32>("", "minute_close")
                    .expect("read restored minute close"),
                restored
                    .try_get::<bool>("", "minute_policy_restored")
                    .expect("read restored minute policy"),
                restored
                    .try_get::<bool>("", "bars_policy_removed")
                    .expect("read removed bars policy"),
            ),
            (Some("1d".into()), Some("7 days".into()), 1, 205, true, true),
        );
        transaction.rollback().await.expect("roll back test schema");
    }
}
