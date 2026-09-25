use migration::{Migrator, MigratorTrait};
use sea_orm::SqlxPostgresConnector;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgConnection, PgPool, query, query_scalar};

pub(super) async fn create_test_pool(pool: PgPool) -> PgPool {
    let connect_options = pool.connect_options().as_ref().clone();
    let pool_options = pool.options().clone();
    let test_database_name = connect_options
        .get_database()
        .expect("sqlx test pool must specify a database")
        .to_string();
    let admin_connect_options = connect_options.clone().database("postgres");

    pool.close().await;

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin_connect_options)
        .await
        .expect("failed to connect to postgres database");
    let mut admin_connection = admin_pool
        .acquire()
        .await
        .expect("failed to acquire postgres connection");

    let migration_names = Migrator::migrations()
        .iter()
        .map(|migration| migration.name())
        .collect::<Vec<_>>()
        .join("\n");
    let migration_hash: String = query_scalar("SELECT md5($1)")
        .bind(migration_names)
        .fetch_one(&mut *admin_connection)
        .await
        .expect("failed to hash migration names");
    let template_database_name = format!("t_rader_test_template_{migration_hash}");
    let staging_database_name = format!("{template_database_name}_building");

    query("SELECT pg_advisory_lock(hashtextextended($1, 0))")
        .bind(&template_database_name)
        .execute(&mut *admin_connection)
        .await
        .expect("failed to lock test database template");

    remove_stale_staging_database(&mut admin_connection, &staging_database_name).await;
    if !database_exists(&mut admin_connection, &template_database_name).await {
        create_template_database(
            &mut admin_connection,
            &connect_options,
            &staging_database_name,
            &template_database_name,
        )
        .await;
    }

    let unlocked: bool = query_scalar("SELECT pg_advisory_unlock(hashtextextended($1, 0))")
        .bind(&template_database_name)
        .fetch_one(&mut *admin_connection)
        .await
        .expect("failed to unlock test database template");
    assert!(unlocked, "test database template lock was not held");

    query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {}",
        quote_identifier(&test_database_name)
    )))
    .execute(&mut *admin_connection)
    .await
    .expect("failed to drop sqlx test database");
    query(AssertSqlSafe(format!(
        "CREATE DATABASE {} TEMPLATE {}",
        quote_identifier(&test_database_name),
        quote_identifier(&template_database_name)
    )))
    .execute(&mut *admin_connection)
    .await
    .expect("failed to clone test database template");

    drop(admin_connection);
    admin_pool.close().await;

    pool_options
        .connect_with(connect_options.database(&test_database_name))
        .await
        .expect("failed to connect to cloned sqlx test database")
}

async fn remove_stale_staging_database(admin_connection: &mut PgConnection, database_name: &str) {
    if !database_exists(admin_connection, database_name).await {
        return;
    }

    disable_connections_and_terminate(admin_connection, database_name).await;
    query(AssertSqlSafe(format!(
        "DROP DATABASE {}",
        quote_identifier(database_name)
    )))
    .execute(admin_connection)
    .await
    .expect("failed to drop stale template staging database");
}

async fn create_template_database(
    admin_connection: &mut PgConnection,
    connect_options: &sqlx::postgres::PgConnectOptions,
    staging_database_name: &str,
    template_database_name: &str,
) {
    query(AssertSqlSafe(format!(
        "CREATE DATABASE {} TEMPLATE template0",
        quote_identifier(staging_database_name)
    )))
    .execute(&mut *admin_connection)
    .await
    .expect("failed to create template staging database");

    let template_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options.clone().database(staging_database_name))
        .await
        .expect("failed to connect to template staging database");
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(template_pool);
    Migrator::up(&db, None)
        .await
        .expect("failed to run template database migrations");
    db.close()
        .await
        .expect("failed to close template database pool");

    disable_connections_and_terminate(admin_connection, staging_database_name).await;
    query(AssertSqlSafe(format!(
        "ALTER DATABASE {} RENAME TO {}",
        quote_identifier(staging_database_name),
        quote_identifier(template_database_name)
    )))
    .execute(&mut *admin_connection)
    .await
    .expect("failed to publish template database");
}

async fn disable_connections_and_terminate(
    admin_connection: &mut PgConnection,
    database_name: &str,
) {
    query(AssertSqlSafe(format!(
        "ALTER DATABASE {} WITH ALLOW_CONNECTIONS false",
        quote_identifier(database_name)
    )))
    .execute(&mut *admin_connection)
    .await
    .expect("failed to disable template database connections");

    let terminated: bool = query_scalar(
        "SELECT COALESCE(bool_and(pg_terminate_backend(pid, 5000)), true) \
         FROM pg_stat_activity \
         WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(database_name)
    .fetch_one(&mut *admin_connection)
    .await
    .expect("failed to terminate template database connections");
    assert!(
        terminated,
        "a template database connection did not terminate"
    );
}

async fn database_exists(admin_connection: &mut PgConnection, database_name: &str) -> bool {
    query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
        .bind(database_name)
        .fetch_one(admin_connection)
        .await
        .expect("failed to check for a test database")
}

fn quote_identifier(identifier: &str) -> String {
    // DDL の識別子は bind できないため、引用してから AssertSqlSafe に渡す。
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
