use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

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
        db.execute_unprepared("SELECT add_compression_policy('minute_bars', INTERVAL '7 days')")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("SELECT remove_compression_policy('minute_bars')")
            .await?;

        manager
            .drop_table(Table::drop().table(MinuteBars::Table).cascade().to_owned())
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

    #[derive(Debug, PartialEq, Eq)]
    struct HypertableSettings {
        compression_enabled: bool,
        chunk_interval: Option<String>,
        segmentby_columns: Option<String>,
        compress_after: Option<String>,
        primary_key_columns: Option<String>,
        foreign_key_definition: Option<String>,
    }

    #[tokio::test]
    async fn migration_creates_and_removes_compressed_minute_bars_hypertable() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let database = Database::connect(database_url)
            .await
            .expect("connect to test database");
        database
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
            .await
            .expect("enable TimescaleDB");

        let transaction = database.begin().await.expect("begin transaction");
        let schema = format!("minute_bars_test_{}", std::process::id());
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

        Migration
            .up(&SchemaManager::new(&transaction))
            .await
            .expect("run minute bars migration");

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
                              AND hypertable_name = 'minute_bars'
                              AND column_name = 'timestamp'
                        ) AS chunk_interval,
                        (
                            SELECT string_agg(attname::text, ',' ORDER BY segmentby_column_index)
                            FROM timescaledb_information.compression_settings
                            WHERE hypertable_schema = $1
                              AND hypertable_name = 'minute_bars'
                              AND segmentby_column_index > 0
                        ) AS segmentby_columns,
                        (
                            SELECT config ->> 'compress_after'
                            FROM timescaledb_information.jobs
                            WHERE hypertable_schema = $1
                              AND hypertable_name = 'minute_bars'
                              AND proc_name = 'policy_compression'
                        ) AS compress_after,
                        (
                            SELECT string_agg(kcu.column_name, ',' ORDER BY kcu.ordinal_position)
                            FROM information_schema.table_constraints AS tc
                            JOIN information_schema.key_column_usage AS kcu
                              ON kcu.constraint_catalog = tc.constraint_catalog
                             AND kcu.constraint_schema = tc.constraint_schema
                             AND kcu.constraint_name = tc.constraint_name
                             AND kcu.table_schema = tc.table_schema
                             AND kcu.table_name = tc.table_name
                            WHERE tc.table_schema = $1
                              AND tc.table_name = 'minute_bars'
                              AND tc.constraint_type = 'PRIMARY KEY'
                        ) AS primary_key_columns,
                        (
                            SELECT string_agg(
                                concat(
                                    kcu.column_name,
                                    '->',
                                    ccu.table_name,
                                    '.',
                                    ccu.column_name,
                                    ':',
                                    rc.delete_rule
                                ),
                                ',' ORDER BY kcu.ordinal_position
                            )
                            FROM information_schema.table_constraints AS tc
                            JOIN information_schema.key_column_usage AS kcu
                              ON kcu.constraint_catalog = tc.constraint_catalog
                             AND kcu.constraint_schema = tc.constraint_schema
                             AND kcu.constraint_name = tc.constraint_name
                             AND kcu.table_schema = tc.table_schema
                             AND kcu.table_name = tc.table_name
                            JOIN information_schema.constraint_column_usage AS ccu
                              ON ccu.constraint_catalog = tc.constraint_catalog
                             AND ccu.constraint_schema = tc.constraint_schema
                             AND ccu.constraint_name = tc.constraint_name
                            JOIN information_schema.referential_constraints AS rc
                              ON rc.constraint_catalog = tc.constraint_catalog
                             AND rc.constraint_schema = tc.constraint_schema
                             AND rc.constraint_name = tc.constraint_name
                            WHERE tc.table_schema = $1
                              AND tc.table_name = 'minute_bars'
                              AND tc.constraint_type = 'FOREIGN KEY'
                        ) AS foreign_key_definition
                    FROM timescaledb_information.hypertables
                    WHERE hypertable_schema = $1
                      AND hypertable_name = 'minute_bars'"
                },
                [schema.clone().into()],
            ))
            .await
            .expect("read hypertable settings")
            .expect("hypertable settings row");
        let settings = HypertableSettings {
            compression_enabled: settings
                .try_get("", "compression_enabled")
                .expect("read compression status"),
            chunk_interval: settings
                .try_get("", "chunk_interval")
                .expect("read chunk interval"),
            segmentby_columns: settings
                .try_get("", "segmentby_columns")
                .expect("read segmentby columns"),
            compress_after: settings
                .try_get("", "compress_after")
                .expect("read compression policy"),
            primary_key_columns: settings
                .try_get("", "primary_key_columns")
                .expect("read primary key columns"),
            foreign_key_definition: settings
                .try_get("", "foreign_key_definition")
                .expect("read foreign key definition"),
        };

        assert_eq!(
            settings,
            HypertableSettings {
                compression_enabled: true,
                chunk_interval: Some("1 day".into()),
                segmentby_columns: Some("instrument_id".into()),
                compress_after: Some("7 days".into()),
                primary_key_columns: Some("instrument_id,timestamp".into()),
                foreign_key_definition: Some("instrument_id->instruments.id:CASCADE".into()),
            },
        );

        Migration
            .down(&SchemaManager::new(&transaction))
            .await
            .expect("reverse minute bars migration");

        let removal = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                indoc::indoc! {
                    "SELECT
                        to_regclass('minute_bars') IS NULL AS table_removed,
                        NOT EXISTS (
                            SELECT 1
                            FROM timescaledb_information.jobs
                            WHERE hypertable_schema = $1
                              AND hypertable_name = 'minute_bars'
                              AND proc_name = 'policy_compression'
                        ) AS policy_removed"
                },
                [schema.into()],
            ))
            .await
            .expect("read migration reversal")
            .expect("migration reversal row");

        assert_eq!(
            (
                removal
                    .try_get::<bool>("", "table_removed")
                    .expect("read table removal"),
                removal
                    .try_get::<bool>("", "policy_removed")
                    .expect("read policy removal"),
            ),
            (true, true),
        );
        transaction.rollback().await.expect("roll back test schema");
    }
}
