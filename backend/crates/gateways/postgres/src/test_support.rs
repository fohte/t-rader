#![allow(
    clippy::expect_used,
    reason = "テスト準備の失敗はテストを即時に失敗させるため"
)]

use std::str::FromStr;

use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, SqlxPostgresConnector, TransactionTrait};
use sqlx::{
    AssertSqlSafe, Connection as _, PgConnection, postgres::PgConnectOptions,
    postgres::PgPoolOptions,
};

use crate::DatabaseHandle;

/// テストごとに独立した rollback transaction を作る。
pub async fn create_test_transaction() -> DatabaseHandle {
    let pool = connect_test_database_or_initialize()
        .await
        .expect("connect to shared test database");
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let transaction = DatabaseHandle::from(db.begin().await.expect("begin test transaction"));
    transaction
        .execute_unprepared(
            "SELECT pg_advisory_xact_lock(hashtext('t-rader-test-database'), hashtext('test-execution'))",
        )
        .await
        .expect("serialize shared database tests");
    transaction
}

async fn connect_test_database() -> Result<sqlx::PgPool, sqlx::Error> {
    connect_pool(test_database_options()).await
}

async fn connect_pool(options: PgConnectOptions) -> Result<sqlx::PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
}

async fn connect_test_database_or_initialize() -> Result<sqlx::PgPool, sqlx::Error> {
    match connect_test_database().await {
        Ok(pool) => Ok(pool),
        Err(error) if is_missing_database(&error) => {
            initialize_test_database().await;
            connect_test_database().await
        }
        Err(error) => Err(error),
    }
}

fn is_missing_database(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "3D000")
}

async fn initialize_test_database() {
    let test_database = test_database_name();
    let initializing_database = initializing_database_name();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let base_options = PgConnectOptions::from_str(&database_url).expect("parse DATABASE_URL");
    let mut admin = PgConnection::connect_with(&base_options.clone().database("postgres"))
        .await
        .expect("connect to PostgreSQL admin database");

    sqlx::query_scalar::<_, bool>(
        "SELECT pg_advisory_lock(hashtext('t-rader-test-database'), hashtext('migration')) IS NULL",
    )
    .fetch_one(&mut admin)
    .await
    .expect("lock shared test database migration");

    let database_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)",
    )
    .bind(&test_database)
    .fetch_one(&mut admin)
    .await
    .expect("check shared test database");
    if !database_exists {
        create_and_publish_test_database(
            &mut admin,
            base_options,
            &initializing_database,
            &test_database,
        )
        .await;
    }

    sqlx::query_scalar::<_, bool>(
        "SELECT pg_advisory_unlock(hashtext('t-rader-test-database'), hashtext('migration'))",
    )
    .fetch_one(&mut admin)
    .await
    .expect("unlock shared test database migration");
}

async fn create_and_publish_test_database(
    admin: &mut PgConnection,
    base_options: PgConnectOptions,
    initializing_database: &str,
    test_database: &str,
) {
    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {} WITH (FORCE)",
        quote_identifier(initializing_database)
    )))
    .execute(&mut *admin)
    .await
    .expect("remove stale shared test database initialization");
    sqlx::query(AssertSqlSafe(format!(
        "CREATE DATABASE {}",
        quote_identifier(initializing_database)
    )))
    .execute(&mut *admin)
    .await
    .expect("create shared test database for migrations");

    let pool = connect_pool(base_options.database(initializing_database))
        .await
        .expect("connect to shared test database for migrations");
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    Migrator::up(&db, None)
        .await
        .expect("run shared test database migrations");
    db.close()
        .await
        .expect("close shared test database after migrations");

    sqlx::query(AssertSqlSafe(format!(
        "ALTER DATABASE {} RENAME TO {}",
        quote_identifier(initializing_database),
        quote_identifier(test_database)
    )))
    .execute(&mut *admin)
    .await
    .expect("publish migrated shared test database");
}

fn test_database_options() -> PgConnectOptions {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnectOptions::from_str(&database_url)
        .expect("parse DATABASE_URL")
        .database(&test_database_name())
}

// 異なる migration source の DB を共存させるため、名前に migration source の hash を含める。
fn test_database_name() -> String {
    format!("t_rader_test_ready_{}", env!("MIGRATION_SOURCE_HASH"))
}

fn initializing_database_name() -> String {
    format!(
        "t_rader_test_initializing_{}",
        env!("MIGRATION_SOURCE_HASH")
    )
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
