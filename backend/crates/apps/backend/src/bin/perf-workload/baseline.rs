use std::env;

use chrono::{Days, NaiveDate};
use gateway_postgres::performance_workload_sql::unoptimized_group_movers_sql;
use rust_decimal::Decimal;
use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, RuntimeErr, Statement};

use crate::connection::{connect_performance_database, error_kind, reference_date_statement};

pub(super) async fn run() -> Result<(), String> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required for the performance control".to_string())?;
    let connection = connect_performance_database(database_url, 8).await?;
    connection
        .execute_unprepared("SET application_name = 'unoptimized-group-movers-control'")
        .await
        .map_err(|error| {
            format!(
                "could not label the performance control connection: {}",
                error_kind(&error)
            )
        })?;

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
        Err(error) => Err(format!(
            "unoptimized group movers SQL failed for a reason other than timeout: {}",
            error_kind(&error)
        )),
    }
}

async fn reference_date(connection: &sea_orm::DatabaseConnection) -> Result<NaiveDate, String> {
    let row = connection
        .query_one_raw(reference_date_statement())
        .await
        .map_err(|error| {
            format!(
                "could not read the performance profile reference date: {}",
                error_kind(&error)
            )
        })?
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
