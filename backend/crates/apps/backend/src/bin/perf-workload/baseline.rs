use std::{env, time::Duration};

use chrono::{Days, NaiveDate};
use gateway_postgres::performance_workload_sql::unoptimized_group_movers_sql;
use rust_decimal::Decimal;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DbErr, RuntimeErr, Statement,
};

pub(super) async fn run() -> Result<(), String> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required for the performance control".to_string())?;
    let mut options = ConnectOptions::new(database_url);
    options.max_connections(1);
    options.min_connections(1);
    options.connect_timeout(Duration::from_secs(30));
    options.sqlx_logging(false);
    options.map_sqlx_postgres_opts(|options| options.statement_cache_capacity(8));
    let connection = Database::connect(options)
        .await
        .map_err(|_| "could not connect to the performance database".to_string())?;
    connection
        .execute_unprepared("SET TIME ZONE 'UTC'")
        .await
        .map_err(|_| "could not set the performance database timezone".to_string())?;
    connection
        .execute_unprepared("SET default_transaction_read_only = on")
        .await
        .map_err(|_| "could not make the performance control read-only".to_string())?;
    connection
        .execute_unprepared("SET statement_timeout = '5s'")
        .await
        .map_err(|_| "could not apply the five second SQL limit".to_string())?;
    connection
        .execute_unprepared("SET plan_cache_mode = auto")
        .await
        .map_err(|_| "could not enable automatic prepared statement planning".to_string())?;
    connection
        .execute_unprepared("SET application_name = 'unoptimized-group-movers-control'")
        .await
        .map_err(|_| "could not label the performance control connection".to_string())?;

    let reference_date = reference_date(&connection).await?;
    let from = reference_date
        .checked_sub_days(Days::new(60))
        .ok_or_else(|| "reference date is outside the supported range".to_string())?;
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        unoptimized_group_movers_sql(),
        [
            "demo-axis".into(),
            from.into(),
            reference_date.into(),
            Decimal::ZERO.into(),
            5_i64.into(),
        ],
    );
    let started = std::time::Instant::now();
    match connection.query_all_raw(statement).await {
        Ok(_) => Err("unoptimized group movers SQL passed the five second control".to_string()),
        Err(error) if is_statement_timeout(&error) => {
            println!(
                "Verified the unoptimized group movers SQL exceeds the five second limit ({} ms).",
                started.elapsed().as_millis()
            );
            Ok(())
        }
        Err(_) => {
            Err("unoptimized group movers SQL failed for a reason other than timeout".to_string())
        }
    }
}

async fn reference_date(connection: &sea_orm::DatabaseConnection) -> Result<NaiveDate, String> {
    let row = connection
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT MAX(latest_timestamp AT TIME ZONE 'UTC')::date AS reference_date \
             FROM bars_timestamp_coverage WHERE timeframe = '1d'"
                .to_string(),
        ))
        .await
        .map_err(|_| "could not read the performance profile reference date".to_string())?
        .ok_or_else(|| "performance profile has no daily bars".to_string())?;
    row.try_get("", "reference_date")
        .map_err(|_| "could not decode the performance profile reference date".to_string())
}

fn is_statement_timeout(error: &DbErr) -> bool {
    matches!(
        error,
        DbErr::Query(RuntimeErr::SqlxError(error)) | DbErr::Exec(RuntimeErr::SqlxError(error))
            if error
                .as_database_error()
                .and_then(|database_error| database_error.code())
                .is_some_and(|code| code.as_ref() == "57014")
    )
}
