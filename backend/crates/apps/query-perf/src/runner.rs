use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use chrono::{Duration as ChronoDuration, NaiveDate};
use sqlx::{Connection as _, PgConnection};

use crate::log::{ExcludedStatement, Exclusion, ParsedLog};

const STATEMENT_TIMEOUT: &str = "5s";

#[derive(Debug)]
pub struct PerfReport {
    pub target_date: NaiveDate,
    pub statements: Vec<StatementReport>,
    pub excluded: Vec<ExcludedStatement>,
    pub failures: Vec<Failure>,
}

#[derive(Debug)]
pub struct StatementReport {
    pub sql: String,
    pub max_duration: Duration,
    pub runs: usize,
    pub succeeded: bool,
}

#[derive(Debug)]
pub struct Failure {
    pub sql: String,
    pub parameters: String,
    pub shifted_parameters: bool,
    pub duration: Duration,
    pub kind: String,
    pub message: String,
}

struct StatementState {
    max_duration: Duration,
    runs: usize,
    succeeded: bool,
}

struct RunError {
    message: String,
    duration: Duration,
    fatal: bool,
    kind: String,
    non_replayable: bool,
}

pub async fn replay(
    database_url: &str,
    parsed_log: ParsedLog,
    exclusions: Vec<Exclusion>,
) -> Result<PerfReport, sqlx::Error> {
    let mut connection = PgConnection::connect(database_url).await?;
    let target_date = latest_bar_date(&mut connection).await?;
    let mut report = PerfReport {
        target_date,
        statements: Vec::new(),
        excluded: parsed_log.excluded,
        failures: Vec::new(),
    };
    let mut statement_states = BTreeMap::<String, StatementState>::new();

    'statements: for (statement_index, (sql, parameter_sets)) in
        parsed_log.statements.into_iter().enumerate()
    {
        let statement_name = format!("query_perf_{statement_index}");
        let first_parameters = parameter_sets.first().cloned().unwrap_or_default();
        let prepare_started = Instant::now();
        let parameter_types = match prepare_statement(
            &mut connection,
            &statement_name,
            &sql,
            &first_parameters,
        )
        .await
        {
            Ok(types) => types,
            Err(error) => {
                if is_non_replayable_error(&error) && exclusion_for(&exclusions, &sql).is_some() {
                    add_exclusion(&mut report.excluded, &exclusions, &sql);
                    continue;
                }
                let fatal = !is_database_error(&error);
                report.failures.push(Failure {
                    sql: sql.clone(),
                    parameters: format_parameters(&first_parameters),
                    shifted_parameters: false,
                    duration: prepare_started.elapsed(),
                    kind: error_kind(&error),
                    message: error.to_string(),
                });
                statement_states.insert(
                    sql,
                    StatementState {
                        max_duration: prepare_started.elapsed(),
                        runs: 0,
                        succeeded: false,
                    },
                );
                if fatal {
                    break 'statements;
                }
                continue;
            }
        };

        let mut shifted_parameter_sets = BTreeSet::new();
        let mut unshiftable = false;
        for parameters in parameter_sets {
            match shift_parameters(&parameters, &parameter_types, target_date) {
                Ok(shifted) => {
                    shifted_parameter_sets.insert(shifted);
                }
                Err(reason) => {
                    if let Some(exclusion) = exclusion_for(&exclusions, &sql) {
                        if !report
                            .excluded
                            .iter()
                            .any(|entry| entry.sql == sql && entry.reason == exclusion.reason)
                        {
                            report.excluded.push(ExcludedStatement {
                                sql: sql.clone(),
                                reason: exclusion.reason.clone(),
                            });
                        }
                    } else {
                        unshiftable = true;
                        report.failures.push(Failure {
                            sql: sql.clone(),
                            parameters: format_parameters(&parameters),
                            shifted_parameters: false,
                            duration: Duration::ZERO,
                            kind: "parameter replay error".to_string(),
                            message: reason,
                        });
                    }
                }
            }
        }
        if unshiftable {
            statement_states.insert(
                sql,
                StatementState {
                    max_duration: Duration::ZERO,
                    runs: 0,
                    succeeded: false,
                },
            );
            continue;
        }

        let mut excluded_as_non_replayable = false;
        for parameters in shifted_parameter_sets {
            let execute_sql = match build_execute_sql(&statement_name, &parameters) {
                Ok(execute_sql) => execute_sql,
                Err(message) => {
                    if exclusion_for(&exclusions, &sql).is_some() {
                        add_exclusion(&mut report.excluded, &exclusions, &sql);
                        continue;
                    }
                    report.failures.push(Failure {
                        sql: sql.clone(),
                        parameters: format_parameters(&parameters),
                        shifted_parameters: true,
                        duration: Duration::ZERO,
                        kind: "parameter replay error".to_string(),
                        message,
                    });
                    statement_states
                        .entry(sql.clone())
                        .and_modify(|state| state.succeeded = false)
                        .or_insert(StatementState {
                            max_duration: Duration::ZERO,
                            runs: 0,
                            succeeded: false,
                        });
                    continue;
                }
            };

            match execute_read_only(&mut connection, &execute_sql).await {
                Ok(duration) => {
                    let state = statement_states
                        .entry(sql.clone())
                        .or_insert(StatementState {
                            max_duration: Duration::ZERO,
                            runs: 0,
                            succeeded: true,
                        });
                    state.max_duration = state.max_duration.max(duration);
                    state.runs += 1;
                }
                Err(error) => {
                    if error.non_replayable && exclusion_for(&exclusions, &sql).is_some() {
                        add_exclusion(&mut report.excluded, &exclusions, &sql);
                        excluded_as_non_replayable = true;
                        break;
                    }
                    let fatal = error.fatal;
                    let state = statement_states
                        .entry(sql.clone())
                        .or_insert(StatementState {
                            max_duration: Duration::ZERO,
                            runs: 0,
                            succeeded: false,
                        });
                    state.max_duration = state.max_duration.max(error.duration);
                    state.succeeded = false;
                    report.failures.push(Failure {
                        sql: sql.clone(),
                        parameters: format_parameters(&parameters),
                        shifted_parameters: true,
                        duration: error.duration,
                        kind: error.kind,
                        message: error.message,
                    });
                    if fatal {
                        break 'statements;
                    }
                }
            }
        }
        if excluded_as_non_replayable {
            statement_states.remove(&sql);
        }
    }

    report.statements = statement_states
        .into_iter()
        .map(|(sql, state)| StatementReport {
            sql,
            max_duration: state.max_duration,
            runs: state.runs,
            succeeded: state.succeeded,
        })
        .collect();
    report.statements.sort_by(|left, right| {
        right
            .max_duration
            .cmp(&left.max_duration)
            .then_with(|| left.sql.cmp(&right.sql))
    });
    Ok(report)
}

async fn latest_bar_date(connection: &mut PgConnection) -> Result<NaiveDate, sqlx::Error> {
    let date = sqlx::query_scalar::<_, Option<String>>(
        "SELECT max(timestamp)::date::text FROM bars WHERE timeframe = '1d'",
    )
    .fetch_one(connection)
    .await?;

    date.and_then(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok())
        .ok_or_else(|| sqlx::Error::Protocol("perf-db has no daily bar date".to_string()))
}

async fn prepare_statement(
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

async fn execute_read_only(
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

fn shift_parameters(
    parameters: &[Option<String>],
    parameter_types: &[String],
    target_date: NaiveDate,
) -> Result<Vec<Option<String>>, String> {
    if parameters.len() != parameter_types.len() {
        return Err(format!(
            "log contains {} parameter(s), while PostgreSQL inferred {} parameter type(s)",
            parameters.len(),
            parameter_types.len()
        ));
    }

    let kinds: Vec<_> = parameter_types
        .iter()
        .map(|type_name| date_parameter_kind(type_name))
        .collect();
    let mut latest_date = None;
    for (parameter, kind) in parameters.iter().zip(&kinds) {
        let (Some(value), Some(kind)) = (parameter, kind) else {
            continue;
        };
        for date in parameter_dates(value, *kind)? {
            latest_date = Some(latest_date.map_or(date, |current: NaiveDate| current.max(date)));
        }
    }
    let day_shift = latest_date
        .map(|date| target_date.signed_duration_since(date).num_days())
        .unwrap_or(0);

    parameters
        .iter()
        .zip(kinds)
        .map(|(parameter, kind)| match (parameter, kind) {
            (Some(value), Some(kind)) => shift_parameter_value(value, kind, day_shift).map(Some),
            (value, _) => Ok(value.clone()),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum DateParameterKind {
    Date,
    Timestamp,
    Array,
}

fn date_parameter_kind(type_name: &str) -> Option<DateParameterKind> {
    let normalized = type_name.trim().to_ascii_lowercase();
    let is_array = normalized.ends_with("[]") || normalized.starts_with('_');
    let base = if let Some(base) = normalized.strip_suffix("[]") {
        base
    } else if is_array {
        normalized.strip_prefix('_').unwrap_or(&normalized)
    } else {
        &normalized
    };
    let kind = match base {
        "date" | "pg_catalog.date" => DateParameterKind::Date,
        "timestamp"
        | "timestamp without time zone"
        | "timestamptz"
        | "timestamp with time zone"
        | "pg_catalog.timestamp"
        | "pg_catalog.timestamptz" => DateParameterKind::Timestamp,
        _ => return None,
    };
    Some(if is_array {
        DateParameterKind::Array
    } else {
        kind
    })
}

fn parameter_dates(value: &str, kind: DateParameterKind) -> Result<Vec<NaiveDate>, String> {
    match kind {
        DateParameterKind::Date => Ok(vec![parse_date_prefix(value, true)?]),
        DateParameterKind::Timestamp => Ok(vec![parse_date_prefix(value, false)?]),
        DateParameterKind::Array => Ok(iso_dates(value)),
    }
}

fn parse_date_prefix(value: &str, date_only: bool) -> Result<NaiveDate, String> {
    let date_text = value
        .get(..10)
        .ok_or_else(|| format!("date parameter is shorter than 10 characters: {value:?}"))?;
    let date = NaiveDate::parse_from_str(date_text, "%Y-%m-%d")
        .map_err(|error| format!("cannot parse date parameter {value:?}: {error}"))?;
    let suffix = value
        .get(10..)
        .ok_or_else(|| format!("invalid UTF-8 boundary in date parameter {value:?}"))?;
    if date_only && !suffix.is_empty() {
        return Err(format!("expected a date parameter, got {value:?}"));
    }
    Ok(date)
}

fn iso_dates(value: &str) -> Vec<NaiveDate> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 {
        return Vec::new();
    }
    (0..=bytes.len() - 10)
        .filter_map(|start| {
            let candidate = value.get(start..start + 10)?;
            if !candidate
                .as_bytes()
                .iter()
                .enumerate()
                .all(|(index, byte)| match index {
                    4 | 7 => *byte == b'-',
                    _ => byte.is_ascii_digit(),
                })
            {
                return None;
            }
            NaiveDate::parse_from_str(candidate, "%Y-%m-%d").ok()
        })
        .collect()
}

fn shift_parameter_value(
    value: &str,
    kind: DateParameterKind,
    day_shift: i64,
) -> Result<String, String> {
    match kind {
        DateParameterKind::Date => shift_date_prefix(value, day_shift, true),
        DateParameterKind::Timestamp => shift_date_prefix(value, day_shift, false),
        DateParameterKind::Array => shift_date_array(value, day_shift),
    }
}

fn shift_date_prefix(value: &str, day_shift: i64, date_only: bool) -> Result<String, String> {
    let date = parse_date_prefix(value, date_only)?;
    let shifted = date
        .checked_add_signed(ChronoDuration::days(day_shift))
        .ok_or_else(|| format!("date shift is out of range for {value:?}"))?;
    let suffix = value
        .get(10..)
        .ok_or_else(|| format!("invalid UTF-8 boundary in date parameter {value:?}"))?;
    Ok(format!("{}{}", shifted.format("%Y-%m-%d"), suffix))
}

fn shift_date_array(value: &str, day_shift: i64) -> Result<String, String> {
    let bytes = value.as_bytes();
    if bytes.len() < 10 {
        return Ok(value.to_string());
    }

    let mut output = String::with_capacity(value.len());
    let mut copied_until = 0;
    for start in 0..=bytes.len() - 10 {
        let Some(candidate) = value.get(start..start + 10) else {
            continue;
        };
        if !candidate
            .as_bytes()
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                4 | 7 => *byte == b'-',
                _ => byte.is_ascii_digit(),
            })
        {
            continue;
        }
        let Ok(date) = NaiveDate::parse_from_str(candidate, "%Y-%m-%d") else {
            continue;
        };
        let shifted = date
            .checked_add_signed(ChronoDuration::days(day_shift))
            .ok_or_else(|| format!("date shift is out of range for {value:?}"))?;
        output.push_str(
            value.get(copied_until..start).ok_or_else(|| {
                format!("invalid UTF-8 boundary in date array parameter {value:?}")
            })?,
        );
        output.push_str(&shifted.format("%Y-%m-%d").to_string());
        copied_until = start + 10;
    }
    output.push_str(
        value
            .get(copied_until..)
            .ok_or_else(|| format!("invalid UTF-8 boundary in date array parameter {value:?}"))?,
    );
    Ok(output)
}

fn build_execute_sql(
    statement_name: &str,
    parameters: &[Option<String>],
) -> Result<String, String> {
    if parameters.is_empty() {
        return Ok(format!("EXECUTE {statement_name}"));
    }

    let values = parameters
        .iter()
        .map(|parameter| match parameter {
            None => Ok("NULL".to_string()),
            Some(value) => dollar_quote(value),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("EXECUTE {statement_name}({})", values.join(", ")))
}

fn dollar_quote(value: &str) -> Result<String, String> {
    if value.contains('\0') {
        return Err("PostgreSQL text literals cannot contain a NUL byte".to_string());
    }

    let mut index = 0;
    loop {
        let tag = if index == 0 {
            "$query_perf$".to_string()
        } else {
            format!("$query_perf_{index}$")
        };
        if !value.contains(&tag) {
            return Ok(format!("{tag}{value}{tag}"));
        }
        index += 1;
    }
}

fn format_parameters(parameters: &[Option<String>]) -> String {
    format!("{parameters:?}")
}

fn exclusion_for<'a>(exclusions: &'a [Exclusion], sql: &str) -> Option<&'a Exclusion> {
    exclusions.iter().find(|entry| entry.sql == sql)
}

fn add_exclusion(report: &mut Vec<ExcludedStatement>, exclusions: &[Exclusion], sql: &str) {
    if report.iter().any(|entry| entry.sql == sql) {
        return;
    }
    if let Some(exclusion) = exclusion_for(exclusions, sql) {
        report.push(ExcludedStatement {
            sql: exclusion.sql.clone(),
            reason: exclusion.reason.clone(),
        });
    }
}

fn error_kind(error: &sqlx::Error) -> String {
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

fn is_database_error(error: &sqlx::Error) -> bool {
    error.as_database_error().is_some()
}

fn is_non_replayable_error(error: &sqlx::Error) -> bool {
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
        DateParameterKind, date_parameter_kind, explicit_parameter_types,
        is_non_replayable_sqlstate, resolve_ambiguous_parameter_types, shift_date_prefix,
        shift_parameters, timestamp_type_hint,
    };
    use chrono::NaiveDate;
    use rstest::rstest;

    #[rstest]
    #[case::date("date", Some("2020-02-03"), "2026-10-16")]
    #[case::timestamp(
        "timestamp without time zone",
        Some("2020-02-03 09:30:00.123"),
        "2026-10-16 09:30:00.123"
    )]
    #[case::timestamptz(
        "timestamp with time zone",
        Some("2020-02-03 09:30:00+00"),
        "2026-10-16 09:30:00+00"
    )]
    fn shifts_typed_dates_to_the_perf_database_end_date(
        #[case] type_name: &str,
        #[case] value: Option<&str>,
        #[case] expected: &str,
    ) -> Result<(), std::io::Error> {
        let target_date =
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?;
        let shifted = shift_parameters(
            &[value.map(str::to_string)],
            &[type_name.to_string()],
            target_date,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(shifted, vec![Some(expected.to_string())]);
        Ok(())
    }

    #[test]
    fn shifts_every_date_in_a_date_array_by_one_shared_day_offset() -> Result<(), std::io::Error> {
        let shifted = shift_parameters(
            &[Some("{2020-02-02,2020-02-03}".to_string())],
            &["date[]".to_string()],
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(shifted, vec![Some("{2026-10-15,2026-10-16}".to_string())]);
        Ok(())
    }

    #[test]
    fn keeps_non_date_parameters_and_null_dates_unchanged() -> Result<(), std::io::Error> {
        let shifted = shift_parameters(
            &[
                Some("2021-01-03".to_string()),
                None,
                Some("record-7".to_string()),
            ],
            &["text".to_string(), "date".to_string(), "text".to_string()],
            NaiveDate::parse_from_str("2026-10-16", "%Y-%m-%d").map_err(std::io::Error::other)?,
        )
        .map_err(std::io::Error::other)?;

        assert_eq!(
            shifted,
            vec![
                Some("2021-01-03".to_string()),
                None,
                Some("record-7".to_string()),
            ]
        );
        Ok(())
    }

    #[test]
    fn recognizes_postgres_date_type_names() {
        assert_eq!(
            date_parameter_kind("timestamp with time zone[]")
                .map(|kind| matches!(kind, DateParameterKind::Array)),
            Some(true)
        );
    }

    #[test]
    fn shifts_a_timestamp_value_without_a_time_part() {
        assert_eq!(
            shift_date_prefix("2020-02-03", 1, false),
            Ok("2020-02-04".to_string())
        );
    }

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
