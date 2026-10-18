use std::time::{Duration, Instant};

use chrono::NaiveDate;
use sqlx::PgConnection;

const STATEMENT_TIMEOUT: &str = "5s";

#[derive(Debug)]
pub(super) struct RunError {
    pub(super) message: String,
    pub(super) duration: Duration,
    pub(super) fatal: bool,
    pub(super) kind: String,
    pub(super) non_replayable: bool,
}

pub(super) async fn prepare_statement(
    connection: &mut PgConnection,
    statement_name: &str,
    sql: &str,
    parameters: &[Option<String>],
) -> Result<Vec<String>, sqlx::Error> {
    begin_read_only(connection).await?;

    let query = sql.trim().trim_end_matches(';').trim_end();
    let prepare_sql = format!("PREPARE {statement_name} AS {query}");
    let preparation = async {
        sqlx::raw_sql(sqlx::AssertSqlSafe(prepare_sql))
            .execute(&mut *connection)
            .await?;
        let mut parameter_types = prepared_parameter_types(connection, statement_name).await?;
        if resolve_ambiguous_parameter_types(&parameter_types, parameters) {
            let explicit_types = explicit_parameter_types(&parameter_types, parameters)?;
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DEALLOCATE {statement_name}")))
                .execute(&mut *connection)
                .await?;
            let prepare_sql = format!(
                "PREPARE {statement_name} ({}) AS {query}",
                explicit_types.join(", ")
            );
            sqlx::raw_sql(sqlx::AssertSqlSafe(prepare_sql))
                .execute(&mut *connection)
                .await?;
            parameter_types = prepared_parameter_types(connection, statement_name).await?;
        }
        Ok(parameter_types)
    }
    .await;

    match preparation {
        Ok(parameter_types) => {
            sqlx::query("COMMIT").execute(&mut *connection).await?;
            Ok(parameter_types)
        }
        Err(error) => match sqlx::query("ROLLBACK").execute(&mut *connection).await {
            Ok(_) => Err(error),
            Err(rollback_error) => Err(rollback_error),
        },
    }
}

async fn prepared_parameter_types(
    connection: &mut PgConnection,
    statement_name: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT parameter.parameter_type::text \
         FROM pg_prepared_statements AS prepared, \
              LATERAL unnest(prepared.parameter_types) WITH ORDINALITY \
                AS parameter(parameter_type, position) \
         WHERE prepared.name = $1 \
         ORDER BY parameter.position",
    )
    .bind(statement_name)
    .fetch_all(&mut *connection)
    .await
}

fn resolve_ambiguous_parameter_types(
    parameter_types: &[String],
    parameters: &[Option<String>],
) -> bool {
    parameter_types.len() == parameters.len()
        && parameter_types
            .iter()
            .zip(parameters)
            .any(|(type_name, parameter)| {
                type_name == "interval"
                    && parameter.as_deref().and_then(timestamp_type_hint).is_some()
            })
}

fn explicit_parameter_types(
    parameter_types: &[String],
    parameters: &[Option<String>],
) -> Result<Vec<String>, sqlx::Error> {
    parameter_types
        .iter()
        .zip(parameters)
        .map(|(type_name, parameter)| {
            let resolved_type = if type_name == "interval" {
                parameter
                    .as_deref()
                    .and_then(timestamp_type_hint)
                    .unwrap_or(type_name)
            } else {
                type_name
            };
            if !resolved_type
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_ .[]".contains(character))
            {
                return Err(sqlx::Error::Protocol(format!(
                    "cannot safely render PostgreSQL parameter type {resolved_type:?}"
                )));
            }
            Ok(resolved_type.to_string())
        })
        .collect()
}

fn timestamp_type_hint(value: &str) -> Option<&'static str> {
    let date_text = value.get(..10)?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d").ok()?;
    let bytes = value.as_bytes();
    if !matches!(bytes.get(10), Some(b' ' | b'T')) {
        return None;
    }
    let time = value.get(11..19)?;
    if time.len() != 8
        || !time
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                2 | 5 => *byte == b':',
                _ => byte.is_ascii_digit(),
            })
    {
        return None;
    }
    let suffix = value.get(19..)?;
    if !suffix.is_empty()
        && !suffix.starts_with('.')
        && !suffix.starts_with('+')
        && !suffix.starts_with('-')
        && !suffix.eq_ignore_ascii_case("z")
    {
        return None;
    }

    if suffix.contains('+') || suffix.contains('-') || suffix.eq_ignore_ascii_case("z") {
        Some("timestamp with time zone")
    } else {
        Some("timestamp without time zone")
    }
}

async fn begin_read_only(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::query("BEGIN READ ONLY")
        .execute(&mut *connection)
        .await?;
    let timeout_statement = format!("SET LOCAL statement_timeout = '{STATEMENT_TIMEOUT}'");
    if let Err(error) = sqlx::raw_sql(sqlx::AssertSqlSafe(timeout_statement))
        .execute(&mut *connection)
        .await
    {
        match sqlx::query("ROLLBACK").execute(&mut *connection).await {
            Ok(_) => Err(error),
            Err(rollback_error) => Err(rollback_error),
        }
    } else {
        Ok(())
    }
}

pub(super) async fn execute_read_only(
    connection: &mut PgConnection,
    execute_sql: &str,
) -> Result<Duration, RunError> {
    if let Err(error) = begin_read_only(connection).await {
        return Err(RunError {
            message: error.to_string(),
            duration: Duration::ZERO,
            fatal: !is_database_error(&error),
            kind: error_kind(&error),
            non_replayable: is_non_replayable_error(&error),
        });
    }

    let started = Instant::now();
    let execution = sqlx::raw_sql(sqlx::AssertSqlSafe(execute_sql.to_string()))
        .execute(&mut *connection)
        .await;
    let duration = started.elapsed();
    let rollback = sqlx::query("ROLLBACK").execute(&mut *connection).await;

    match (execution, rollback) {
        (Ok(_), Ok(_)) => Ok(duration),
        (Err(error), Ok(_)) => Err(RunError {
            message: error.to_string(),
            duration,
            fatal: !is_database_error(&error),
            kind: error_kind(&error),
            non_replayable: is_non_replayable_error(&error),
        }),
        (Ok(_), Err(error)) => Err(RunError {
            message: format!("query completed but rollback failed: {error}"),
            duration,
            fatal: true,
            kind: error_kind(&error),
            non_replayable: false,
        }),
        (Err(error), Err(rollback_error)) => Err(RunError {
            message: format!("{error}; rollback failed: {rollback_error}"),
            duration,
            fatal: true,
            kind: error_kind(&error),
            non_replayable: false,
        }),
    }
}

pub(super) fn error_kind(error: &sqlx::Error) -> String {
    if error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "57014")
    {
        "statement timeout".to_string()
    } else if is_database_error(error) {
        "execution error".to_string()
    } else {
        "connection error (possible OOM disconnect)".to_string()
    }
}

pub(super) fn is_database_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some()
}

pub(super) fn is_non_replayable_error(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| is_non_replayable_sqlstate(code.as_ref()))
}

fn is_non_replayable_sqlstate(code: &str) -> bool {
    matches!(code, "25006" | "42P01" | "42703")
}

#[cfg(test)]
mod tests {
    use super::{
        explicit_parameter_types, is_non_replayable_sqlstate, resolve_ambiguous_parameter_types,
        timestamp_type_hint,
    };
    use rstest::rstest;

    #[test]
    fn resolves_timestamp_values_that_postgres_inferred_as_intervals() {
        let parameter_types = vec!["interval".to_string()];
        let parameters = vec![Some("2026-10-18 20:27:19.720876+00".to_string())];

        assert_eq!(
            (
                resolve_ambiguous_parameter_types(&parameter_types, &parameters),
                explicit_parameter_types(&parameter_types, &parameters)
                    .map_err(|error| error.to_string()),
                timestamp_type_hint("2026-10-18 20:27:19.720876+00"),
            ),
            (
                true,
                Ok(vec!["timestamp with time zone".to_string()]),
                Some("timestamp with time zone"),
            )
        );
    }
    #[rstest]
    #[case::read_only_transaction("25006", true)]
    #[case::missing_relation("42P01", true)]
    #[case::missing_column("42703", true)]
    #[case::statement_timeout("57014", false)]
    #[case::invalid_input("22007", false)]
    fn allows_only_known_non_replayable_sqlstates(#[case] code: &str, #[case] expected: bool) {
        assert_eq!(is_non_replayable_sqlstate(code), expected);
    }
}
