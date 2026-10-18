mod execution;
mod parameters;

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, pool::PoolConnection, postgres::PgPoolOptions};

use crate::log::{ExcludedStatement, Exclusion, ParsedLog};
use execution::{
    deallocate_statement, error_kind, execute_read_only, is_database_error,
    is_non_replayable_error, prepare_statement, strip_for_update_clause,
};
use parameters::{build_execute_sql, format_parameters, shift_parameters};

const CONNECTION_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(120);
const RECONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(250);

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

impl StatementState {
    fn failed(max_duration: Duration) -> Self {
        Self {
            max_duration,
            runs: 0,
            succeeded: false,
        }
    }
}

pub async fn replay(
    database_url: &str,
    parsed_log: ParsedLog,
    exclusions: Vec<Exclusion>,
) -> Result<PerfReport, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(CONNECTION_ACQUIRE_TIMEOUT)
        .connect(database_url)
        .await?;
    let target_date = latest_bar_date(&pool).await?;
    let mut report = PerfReport {
        target_date,
        statements: Vec::new(),
        excluded: parsed_log.excluded,
        failures: Vec::new(),
    };
    let mut statement_states = BTreeMap::<String, StatementState>::new();
    let mut stopped_early = false;
    let mut statement_iter = parsed_log.statements.into_iter().enumerate();

    'statements: while let Some((statement_index, (sql, parameter_sets))) = statement_iter.next() {
        let statement_name = format!("query_perf_{statement_index}");
        let replay_sql = match strip_for_update_clause(&sql) {
            Ok(replay_sql) => replay_sql,
            Err(message) => {
                for parameters in parameter_sets {
                    report.failures.push(Failure {
                        sql: sql.clone(),
                        parameters: format_parameters(&parameters),
                        shifted_parameters: false,
                        duration: Duration::ZERO,
                        kind: "query transformation error".to_string(),
                        message: message.clone(),
                    });
                }
                statement_states.insert(sql, StatementState::failed(Duration::ZERO));
                continue;
            }
        };
        let mut excluded_as_non_replayable = false;
        let mut parameter_iter = parameter_sets.into_iter();
        while let Some(parameters) = parameter_iter.next() {
            let acquire_started = Instant::now();
            let mut connection = match acquire_connection(&pool).await {
                Ok(connection) => connection,
                Err(error) => {
                    let duration = acquire_started.elapsed();
                    report.failures.push(Failure {
                        sql: sql.clone(),
                        parameters: format_parameters(&parameters),
                        shifted_parameters: false,
                        duration,
                        kind: "connection unavailable".to_string(),
                        message: format!("could not reconnect to perf-db: {error}"),
                    });
                    statement_states
                        .entry(sql.clone())
                        .and_modify(|state| {
                            state.max_duration = state.max_duration.max(duration);
                            state.succeeded = false;
                        })
                        .or_insert(StatementState::failed(duration));
                    for remaining_parameters in parameter_iter {
                        record_unexecuted_statement(
                            &mut report,
                            &mut statement_states,
                            &sql,
                            &remaining_parameters,
                        );
                    }
                    for (_, (remaining_sql, remaining_parameter_sets)) in statement_iter.by_ref() {
                        for remaining_parameters in remaining_parameter_sets {
                            record_unexecuted_statement(
                                &mut report,
                                &mut statement_states,
                                &remaining_sql,
                                &remaining_parameters,
                            );
                        }
                    }
                    stopped_early = true;
                    break 'statements;
                }
            };

            let prepare_started = Instant::now();
            let parameter_types =
                match prepare_statement(&mut connection, &statement_name, &replay_sql, &parameters)
                    .await
                {
                    Ok(types) => types,
                    Err(error) => {
                        if is_non_replayable_error(&error)
                            && exclusion_for(&exclusions, &sql).is_some()
                        {
                            add_exclusion(&mut report.excluded, &exclusions, &sql);
                            excluded_as_non_replayable = true;
                            break;
                        }
                        let duration = prepare_started.elapsed();
                        report.failures.push(Failure {
                            sql: sql.clone(),
                            parameters: format_parameters(&parameters),
                            shifted_parameters: false,
                            duration,
                            kind: error_kind(&error),
                            message: error.to_string(),
                        });
                        let state = statement_states
                            .entry(sql.clone())
                            .or_insert(StatementState::failed(Duration::ZERO));
                        state.max_duration = state.max_duration.max(duration);
                        state.succeeded = false;
                        if !is_database_error(&error) {
                            let _ = connection.close().await;
                        }
                        continue;
                    }
                };

            let parameters = match shift_parameters(&parameters, &parameter_types, target_date) {
                Ok(parameters) => parameters,
                Err(message) => {
                    if deallocate_statement(&mut connection, &statement_name)
                        .await
                        .is_err()
                    {
                        let _ = connection.close().await;
                    }
                    if exclusion_for(&exclusions, &sql).is_some() {
                        add_exclusion(&mut report.excluded, &exclusions, &sql);
                        excluded_as_non_replayable = true;
                        break;
                    }
                    report.failures.push(Failure {
                        sql: sql.clone(),
                        parameters: format_parameters(&parameters),
                        shifted_parameters: false,
                        duration: Duration::ZERO,
                        kind: "parameter replay error".to_string(),
                        message,
                    });
                    statement_states
                        .entry(sql.clone())
                        .and_modify(|state| state.succeeded = false)
                        .or_insert(StatementState::failed(Duration::ZERO));
                    continue;
                }
            };

            let execute_sql = match build_execute_sql(&statement_name, &parameters) {
                Ok(execute_sql) => execute_sql,
                Err(message) => {
                    if deallocate_statement(&mut connection, &statement_name)
                        .await
                        .is_err()
                    {
                        let _ = connection.close().await;
                    }
                    if exclusion_for(&exclusions, &sql).is_some() {
                        add_exclusion(&mut report.excluded, &exclusions, &sql);
                        excluded_as_non_replayable = true;
                        break;
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
                        .or_insert(StatementState::failed(Duration::ZERO));
                    continue;
                }
            };

            let execution = execute_read_only(&mut connection, &execute_sql).await;
            let execution_fatal = matches!(&execution, Err(error) if error.fatal);
            let cleanup_error = if execution_fatal {
                let _ = connection.close().await;
                None
            } else {
                match deallocate_statement(&mut connection, &statement_name).await {
                    Ok(()) => None,
                    Err(error) => {
                        let _ = connection.close().await;
                        Some(error)
                    }
                }
            };

            match execution {
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
                    let state = statement_states
                        .entry(sql.clone())
                        .or_insert(StatementState::failed(Duration::ZERO));
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
                }
            }
            if let Some(error) = cleanup_error {
                statement_states
                    .entry(sql.clone())
                    .and_modify(|state| state.succeeded = false)
                    .or_insert(StatementState::failed(Duration::ZERO));
                report.failures.push(Failure {
                    sql: sql.clone(),
                    parameters: format_parameters(&parameters),
                    shifted_parameters: true,
                    duration: Duration::ZERO,
                    kind: error_kind(&error),
                    message: format!("failed to deallocate prepared statement: {error}"),
                });
            }
        }
        if excluded_as_non_replayable {
            statement_states.remove(&sql);
        }
    }

    if !stopped_early {
        for exclusion in unused_exclusions(&exclusions, &report.excluded) {
            report.failures.push(Failure {
                sql: exclusion.sql.clone(),
                parameters: "[]".to_string(),
                shifted_parameters: false,
                duration: Duration::ZERO,
                kind: "unused exclusion".to_string(),
                message: "configured exclusion was not needed by any SQL statement".to_string(),
            });
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

async fn latest_bar_date(pool: &PgPool) -> Result<NaiveDate, sqlx::Error> {
    let date = sqlx::query_scalar::<_, Option<String>>(
        "SELECT max(timestamp)::date::text FROM bars WHERE timeframe = '1d'",
    )
    .fetch_one(pool)
    .await?;

    date.and_then(|value| NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok())
        .ok_or_else(|| sqlx::Error::Protocol("perf-db has no daily bar date".to_string()))
}

async fn acquire_connection(pool: &PgPool) -> Result<PoolConnection<Postgres>, sqlx::Error> {
    let started = Instant::now();
    loop {
        match pool.acquire().await {
            Ok(connection) => return Ok(connection),
            Err(error) if started.elapsed() >= RECONNECT_TIMEOUT => return Err(error),
            Err(_) => tokio::time::sleep(RECONNECT_RETRY_INTERVAL).await,
        }
    }
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

fn unused_exclusions<'a>(
    exclusions: &'a [Exclusion],
    used: &[ExcludedStatement],
) -> Vec<&'a Exclusion> {
    exclusions
        .iter()
        .filter(|exclusion| !used.iter().any(|entry| entry.sql == exclusion.sql))
        .collect()
}

fn record_unexecuted_statement(
    report: &mut PerfReport,
    statement_states: &mut BTreeMap<String, StatementState>,
    sql: &str,
    parameters: &[Option<String>],
) {
    report.failures.push(Failure {
        sql: sql.to_string(),
        parameters: format_parameters(parameters),
        shifted_parameters: false,
        duration: Duration::ZERO,
        kind: "connection unavailable".to_string(),
        message: "not executed because perf-db could not be reconnected".to_string(),
    });
    statement_states
        .entry(sql.to_string())
        .and_modify(|state| state.succeeded = false)
        .or_insert(StatementState::failed(Duration::ZERO));
}

#[cfg(test)]
mod tests {
    use super::unused_exclusions;
    use crate::log::{ExcludedStatement, Exclusion};

    #[test]
    fn finds_exclusions_that_were_not_applied_during_replay() {
        let exclusions = vec![
            Exclusion {
                sql: "SELECT 1".to_string(),
                reason: "fixture reason".to_string(),
            },
            Exclusion {
                sql: "SELECT 2".to_string(),
                reason: "fixture reason".to_string(),
            },
        ];
        let used = vec![ExcludedStatement {
            sql: "SELECT 1".to_string(),
            reason: "fixture reason".to_string(),
        }];

        assert_eq!(unused_exclusions(&exclusions, &used), vec![&exclusions[1]]);
    }
}
