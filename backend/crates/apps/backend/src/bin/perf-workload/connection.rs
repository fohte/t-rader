use std::time::Duration;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr,
    RuntimeErr,
};

pub(super) async fn connect_performance_database(
    database_url: String,
    statement_cache_capacity: usize,
) -> Result<DatabaseConnection, String> {
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(1);
    options.min_connections(1);
    options.connect_timeout(Duration::from_secs(30));
    options.sqlx_logging(false);
    options.map_sqlx_postgres_opts(move |options| {
        options.statement_cache_capacity(statement_cache_capacity)
    });
    let connection = Database::connect(options).await.map_err(|error| {
        format!(
            "could not connect to the performance database: {}",
            error_kind(&error)
        )
    })?;

    for (statement, operation) in [
        (
            "SET TIME ZONE 'UTC'",
            "set the performance database timezone",
        ),
        (
            "SET default_transaction_read_only = on",
            "make the performance connection read-only",
        ),
        (
            "SET statement_timeout = '5s'",
            "apply the five second SQL limit",
        ),
        (
            "SET plan_cache_mode = auto",
            "enable automatic prepared statement planning",
        ),
    ] {
        connection
            .execute_unprepared(statement)
            .await
            .map_err(|error| format!("could not {operation}: {}", error_kind(&error)))?;
    }

    Ok(connection)
}

pub(super) fn error_kind(error: &DbErr) -> String {
    let sqlstate = match error {
        DbErr::Exec(RuntimeErr::SqlxError(error)) | DbErr::Query(RuntimeErr::SqlxError(error)) => {
            error
                .as_database_error()
                .and_then(|database_error| database_error.code())
        }
        _ => None,
    };
    sqlstate.map_or_else(
        || "database operation error".to_string(),
        |code| format!("SQLSTATE {code}"),
    )
}

pub(super) fn reference_date_statement() -> sea_orm::Statement {
    sea_orm::Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT MAX(latest_timestamp AT TIME ZONE 'UTC')::date AS reference_date \
         FROM bars_timestamp_coverage WHERE timeframe = '1d'"
            .to_string(),
    )
}
