use migration::{Migrator, MigratorTrait};
use sea_orm::SqlxPostgresConnector;
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgConnection, PgPool, query, query_scalar};

const TEMPLATE_DATABASE_PREFIX: &str = "t_rader_test_template_";
const TEMPLATE_DATABASE_NAME_PATTERN: &str = r"^t_rader_test_template_[0-9a-f]{32}(_building)?$";

/// 注入された pool を閉じ、同名の DB を migration 済み template から作り直す。
pub(super) async fn create_test_pool_from_template(pool: PgPool) -> PgPool {
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
    let migration_key = format!("{migration_names}\n{}", env!("MIGRATION_SOURCE_HASH"));
    let migration_hash: String = query_scalar("SELECT md5($1)")
        .bind(migration_key)
        .fetch_one(&mut *admin_connection)
        .await
        .expect("failed to hash migration sources");
    let template_database_name = format!("{TEMPLATE_DATABASE_PREFIX}{migration_hash}");
    let staging_database_name = format!("{template_database_name}_building");

    ensure_template_and_clone(
        &mut admin_connection,
        &connect_options,
        &test_database_name,
        &template_database_name,
        &staging_database_name,
    )
    .await;

    remove_obsolete_template_databases(&mut admin_connection, &template_database_name).await;
    drop(admin_connection);
    admin_pool.close().await;

    pool_options
        .connect_with(connect_options.database(&test_database_name))
        .await
        .expect("failed to connect to cloned sqlx test database")
}

async fn ensure_template_and_clone(
    admin_connection: &mut PgConnection,
    connect_options: &sqlx::postgres::PgConnectOptions,
    test_database_name: &str,
    template_database_name: &str,
    staging_database_name: &str,
) {
    acquire_shared_template_lock(admin_connection, template_database_name).await;
    if database_exists(admin_connection, template_database_name).await {
        clone_test_database(admin_connection, test_database_name, template_database_name).await;
        release_shared_template_lock(admin_connection, template_database_name).await;
        return;
    }

    release_shared_template_lock(admin_connection, template_database_name).await;
    acquire_template_lock(admin_connection, template_database_name).await;

    remove_stale_staging_database(admin_connection, staging_database_name).await;
    if !database_exists(admin_connection, template_database_name).await {
        create_template_database(
            admin_connection,
            connect_options,
            staging_database_name,
            template_database_name,
        )
        .await;
    }
    clone_test_database(admin_connection, test_database_name, template_database_name).await;
    release_template_lock(admin_connection, template_database_name).await;
}

async fn clone_test_database(
    admin_connection: &mut PgConnection,
    test_database_name: &str,
    template_database_name: &str,
) {
    execute_ddl(
        admin_connection,
        format!(
            "DROP DATABASE IF EXISTS {}",
            quote_identifier(test_database_name)
        ),
        "failed to drop sqlx test database",
    )
    .await;
    execute_ddl(
        admin_connection,
        format!(
            "CREATE DATABASE {} TEMPLATE {}",
            quote_identifier(test_database_name),
            quote_identifier(template_database_name)
        ),
        "failed to clone test database template",
    )
    .await;
}

async fn remove_stale_staging_database(admin_connection: &mut PgConnection, database_name: &str) {
    if !database_exists(admin_connection, database_name).await {
        return;
    }

    disable_connections_and_terminate(admin_connection, database_name).await;
    execute_ddl(
        admin_connection,
        format!("DROP DATABASE {}", quote_identifier(database_name)),
        "failed to drop stale template staging database",
    )
    .await;
}

async fn create_template_database(
    admin_connection: &mut PgConnection,
    connect_options: &sqlx::postgres::PgConnectOptions,
    staging_database_name: &str,
    template_database_name: &str,
) {
    execute_ddl(
        admin_connection,
        format!(
            "CREATE DATABASE {} TEMPLATE template0",
            quote_identifier(staging_database_name)
        ),
        "failed to create template staging database",
    )
    .await;

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
    execute_ddl(
        admin_connection,
        format!(
            "ALTER DATABASE {} RENAME TO {}",
            quote_identifier(staging_database_name),
            quote_identifier(template_database_name)
        ),
        "failed to publish template database",
    )
    .await;
}

async fn disable_connections_and_terminate(
    admin_connection: &mut PgConnection,
    database_name: &str,
) {
    execute_ddl(
        admin_connection,
        format!(
            "ALTER DATABASE {} WITH ALLOW_CONNECTIONS false",
            quote_identifier(database_name)
        ),
        "failed to disable template database connections",
    )
    .await;

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

async fn remove_obsolete_template_databases(
    admin_connection: &mut PgConnection,
    current_template_database_name: &str,
) {
    let database_names: Vec<String> = query_scalar(
        "SELECT datname FROM pg_database \
         WHERE datname ~ $1 AND datname <> $2",
    )
    .bind(TEMPLATE_DATABASE_NAME_PATTERN)
    .bind(current_template_database_name)
    .fetch_all(&mut *admin_connection)
    .await
    .expect("failed to list obsolete template databases");

    for database_name in database_names {
        let template_database_name = database_name
            .strip_suffix("_building")
            .unwrap_or(&database_name);
        if !try_acquire_template_lock(admin_connection, template_database_name).await {
            continue;
        }

        if database_exists(admin_connection, &database_name).await {
            disable_connections_and_terminate(admin_connection, &database_name).await;
            let result = query(AssertSqlSafe(format!(
                "DROP DATABASE IF EXISTS {}",
                quote_identifier(&database_name)
            )))
            .execute(&mut *admin_connection)
            .await;
            if let Err(error) = result {
                tracing::debug!(
                    database_name = %database_name,
                    error = %error,
                    "failed to drop obsolete template database"
                );
            }
        }

        release_template_lock(admin_connection, template_database_name).await;
    }
}

async fn acquire_shared_template_lock(admin_connection: &mut PgConnection, database_name: &str) {
    query("SELECT pg_advisory_lock_shared(hashtextextended($1, 0))")
        .bind(database_name)
        .execute(admin_connection)
        .await
        .expect("failed to acquire shared test database template lock");
}

async fn release_shared_template_lock(admin_connection: &mut PgConnection, database_name: &str) {
    let unlocked: bool = query_scalar("SELECT pg_advisory_unlock_shared(hashtextextended($1, 0))")
        .bind(database_name)
        .fetch_one(admin_connection)
        .await
        .expect("failed to release shared test database template lock");
    assert!(unlocked, "shared test database template lock was not held");
}

async fn acquire_template_lock(admin_connection: &mut PgConnection, database_name: &str) {
    query("SELECT pg_advisory_lock(hashtextextended($1, 0))")
        .bind(database_name)
        .execute(admin_connection)
        .await
        .expect("failed to acquire test database template lock");
}

async fn try_acquire_template_lock(
    admin_connection: &mut PgConnection,
    database_name: &str,
) -> bool {
    query_scalar("SELECT pg_try_advisory_lock(hashtextextended($1, 0))")
        .bind(database_name)
        .fetch_one(admin_connection)
        .await
        .expect("failed to check test database template lock")
}

async fn release_template_lock(admin_connection: &mut PgConnection, database_name: &str) {
    let unlocked: bool = query_scalar("SELECT pg_advisory_unlock(hashtextextended($1, 0))")
        .bind(database_name)
        .fetch_one(admin_connection)
        .await
        .expect("failed to release test database template lock");
    assert!(unlocked, "test database template lock was not held");
}

async fn execute_ddl(admin_connection: &mut PgConnection, sql: String, failure: &'static str) {
    query(AssertSqlSafe(sql))
        .execute(admin_connection)
        .await
        .expect(failure);
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
