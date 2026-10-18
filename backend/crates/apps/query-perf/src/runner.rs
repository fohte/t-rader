mod execution;
mod parameters;

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use chrono::NaiveDate;
use sqlx::{Connection as _, PgConnection};

use crate::log::{ExcludedStatement, Exclusion, ParsedLog};
use execution::{
    error_kind, execute_read_only, is_database_error, is_non_replayable_error, prepare_statement,
};
use parameters::{build_execute_sql, format_parameters, shift_parameters};

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
