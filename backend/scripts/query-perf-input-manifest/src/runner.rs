pub mod parameters {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/apps/query-perf/src/runner/parameters.rs"
    ));
}
pub mod sql_rewrite {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/apps/query-perf/src/runner/sql_rewrite.rs"
    ));
}
pub mod execution {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/apps/query-perf/src/runner/execution.rs"
    ));
}

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, PgConnection};

use crate::{
    EXPECTED_SPECIFIED_GROUP_BIND_HASHES, hash_serialized,
    query_log::{ExcludedStatement, Exclusion, ParsedLog},
    sha256,
};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ExclusionFingerprint {
    pub sql_sha256: String,
    pub reason_sha256: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BindFingerprint {
    pub logged_parameter_count: usize,
    pub prepared_parameter_count: usize,
    pub parameter_types_sha256: String,
    pub shifted_values_sha256: String,
    pub typed_shifted_bind_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SqlFingerprint {
    pub sql_sha256: String,
    pub bind_count: usize,
    pub parameter_types_set_sha256: String,
    pub shifted_bind_set_sha256: String,
    pub typed_shifted_bind_set_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupMoversFingerprint {
    pub sql_sha256: String,
    pub bind_count: usize,
    pub specified_bind_count: usize,
    pub specified_bind_hashes: Vec<String>,
    pub specified_bind_set_sha256: String,
    pub parameter_types_sha256: String,
    pub parameter_types_set_sha256: String,
    pub typed_shifted_bind_set_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RawGroupBind {
    pub parameter_types: Vec<String>,
    pub original_parameters: Vec<Option<String>>,
    pub shifted_parameters: Vec<Option<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InputComparison {
    pub target_date: String,
    pub parsed_sql_count: usize,
    pub unique_raw_bind_count: usize,
    pub sql_set_sha256: String,
    pub sqls: Vec<SqlFingerprint>,
    pub configured_exclusion_count: usize,
    pub configured_exclusion_set_sha256: String,
    pub configured_exclusions: Vec<ExclusionFingerprint>,
    pub applicable_exclusion_count: usize,
    pub applicable_exclusion_set_sha256: String,
    pub applicable_exclusions: Vec<ExclusionFingerprint>,
    pub parser_exclusion_count: usize,
    pub parser_exclusion_set_sha256: String,
    pub parser_exclusions: Vec<ExclusionFingerprint>,
    pub preflight_exclusion_count: usize,
    pub preflight_exclusion_set_sha256: String,
    pub preflight_exclusions: Vec<ExclusionFingerprint>,
    pub all_configured_exclusion_sql_present: bool,
    pub group_movers: Option<GroupMoversFingerprint>,
}

pub async fn fingerprint(
    parsed: ParsedLog,
    exclusions: &[Exclusion],
    database_url: &str,
    target_date: NaiveDate,
) -> Result<(InputComparison, Vec<RawGroupBind>), String> {
    let parser_exclusions = fingerprints_for_parsed_exclusions(&parsed.excluded);
    let configured_exclusions = fingerprints_for_configured_exclusions(exclusions);
    let mut preflight_exclusions: BTreeMap<String, String> = parsed
        .excluded
        .iter()
        .map(|excluded| (excluded.sql.clone(), excluded.reason.clone()))
        .collect();
    let mut connection = PgConnection::connect(database_url)
        .await
        .map_err(|_| "could not connect to the migration-only or test database".to_string())?;
    let mut sqls = Vec::with_capacity(parsed.statements.len());
    let mut group_movers: Option<GroupMoversFingerprint> = None;
    let mut total_bind_count = 0;
    let mut raw_group_binds = Vec::new();

    for (sql_index, (sql, parameter_sets)) in parsed.statements.iter().enumerate() {
        let sql_sha256 = sha256(sql.as_bytes());
        let replay_sql = execution::strip_for_update_clause(sql)
            .map_err(|_| format!("SQL transformation failed for {sql_sha256}"))?;
        let mut binds = Vec::with_capacity(parameter_sets.len());
        let mut group_parameter_types: Option<Vec<String>> = None;
        let mut group_typed_bind_hashes = Vec::new();
        let mut group_specified_binds = BTreeSet::new();

        for (bind_index, parameters) in parameter_sets.iter().enumerate() {
            let statement_name = format!("input_manifest_{sql_index}_{bind_index}");
            let parameter_types = match execution::prepare_statement(
                &mut connection,
                &statement_name,
                &replay_sql,
                parameters,
            )
            .await
            {
                Ok(parameter_types) => parameter_types,
                Err(error) => {
                    let exclusion = exclusions.iter().find(|entry| entry.sql == *sql);
                    if execution::is_non_replayable_error(&error)
                        && let Some(exclusion) = exclusion
                    {
                        if !parameters.is_empty() {
                            return Err(format!(
                                "parameter types are unresolved for excluded SQL {sql_sha256}"
                            ));
                        }
                        preflight_exclusions
                            .insert(exclusion.sql.clone(), exclusion.reason.clone());
                        binds.push(bind_fingerprint(&[], &[])?);
                        continue;
                    }
                    let sqlstate = error
                        .as_database_error()
                        .and_then(sqlx::error::DatabaseError::code);
                    return Err(prepare_failure_detail(&sql_sha256, sqlstate.as_deref()));
                }
            };

            if parameter_types.len() != parameters.len() {
                let _ = execution::deallocate_statement(&mut connection, &statement_name).await;
                let exclusion = exclusions.iter().find(|entry| entry.sql == *sql);
                if parameters.is_empty()
                    && let Some(exclusion) = exclusion
                {
                    preflight_exclusions.insert(exclusion.sql.clone(), exclusion.reason.clone());
                    binds.push(bind_fingerprint(&parameter_types, &parameters)?);
                    continue;
                }
                return Err(format!(
                    "parameter type count mismatch for SQL {sql_sha256}: logged bind count {}, PREPARE inferred type count {}",
                    parameters.len(),
                    parameter_types.len()
                ));
            }

            let shifted_parameters =
                match parameters::shift_parameters(parameters, &parameter_types, target_date) {
                    Ok(values) => values,
                    Err(_) => {
                        let _ =
                            execution::deallocate_statement(&mut connection, &statement_name).await;
                        return Err(format!("parameter shifting failed for SQL {sql_sha256}"));
                    }
                };

            if parameters::build_execute_sql(&statement_name, &shifted_parameters).is_err() {
                let _ = execution::deallocate_statement(&mut connection, &statement_name).await;
                let exclusion = exclusions.iter().find(|entry| entry.sql == *sql);
                if let Some(exclusion) = exclusion {
                    preflight_exclusions.insert(exclusion.sql.clone(), exclusion.reason.clone());
                    binds.push(bind_fingerprint(&parameter_types, &shifted_parameters)?);
                    continue;
                }
                return Err(format!("parameter encoding failed for SQL {sql_sha256}"));
            }

            execution::deallocate_statement(&mut connection, &statement_name)
                .await
                .map_err(|_| format!("prepared statement cleanup failed for SQL {sql_sha256}"))?;

            let fingerprint = bind_fingerprint(&parameter_types, &shifted_parameters)?;
            if is_group_movers_sql(sql) {
                if group_parameter_types
                    .as_ref()
                    .is_some_and(|expected| expected != &parameter_types)
                {
                    return Err(
                        "LIST_GROUP_MOVERS_SQL parameter types changed between binds".to_string(),
                    );
                }
                group_parameter_types = Some(parameter_types.clone());
                group_typed_bind_hashes.push(fingerprint.typed_shifted_bind_sha256.clone());
                if EXPECTED_SPECIFIED_GROUP_BIND_HASHES
                    .contains(&fingerprint.typed_shifted_bind_sha256.as_str())
                {
                    group_specified_binds.insert(fingerprint.typed_shifted_bind_sha256.clone());
                }
                raw_group_binds.push(RawGroupBind {
                    parameter_types,
                    original_parameters: parameters.clone(),
                    shifted_parameters: shifted_parameters.clone(),
                });
            }
            binds.push(fingerprint);
        }

        total_bind_count += parameter_sets.len();
        binds.sort();
        let mut parameter_types_hashes: Vec<_> = binds
            .iter()
            .map(|bind| bind.parameter_types_sha256.as_str())
            .collect();
        let mut shifted_bind_hashes: Vec<_> = binds
            .iter()
            .map(|bind| bind.shifted_values_sha256.as_str())
            .collect();
        let mut typed_bind_hashes: Vec<_> = binds
            .iter()
            .map(|bind| bind.typed_shifted_bind_sha256.as_str())
            .collect();
        parameter_types_hashes.sort_unstable();
        shifted_bind_hashes.sort();
        typed_bind_hashes.sort_unstable();
        let fingerprint = SqlFingerprint {
            sql_sha256: sql_sha256.clone(),
            bind_count: parameter_sets.len(),
            parameter_types_set_sha256: hash_serialized(&parameter_types_hashes)?,
            shifted_bind_set_sha256: hash_serialized(&shifted_bind_hashes)?,
            typed_shifted_bind_set_sha256: hash_serialized(&typed_bind_hashes)?,
        };

        if is_group_movers_sql(sql) {
            if group_movers.is_some() {
                return Err("multiple LIST_GROUP_MOVERS_SQL candidates were found".to_string());
            }
            let parameter_types = group_parameter_types.ok_or_else(|| {
                "LIST_GROUP_MOVERS_SQL parameter types are unresolved".to_string()
            })?;
            let expected_types = ["text", "date", "date", "numeric", "bigint"].map(str::to_string);
            if parameter_types != expected_types {
                return Err(
                    "LIST_GROUP_MOVERS_SQL parameter types do not match PREPARE".to_string()
                );
            }
            group_typed_bind_hashes.sort();
            let specified_bind_hashes: Vec<_> = group_specified_binds.into_iter().collect();
            group_movers = Some(GroupMoversFingerprint {
                sql_sha256,
                bind_count: parameter_sets.len(),
                specified_bind_count: specified_bind_hashes.len(),
                specified_bind_set_sha256: hash_serialized(&specified_bind_hashes)?,
                specified_bind_hashes,
                parameter_types_sha256: hash_serialized(&parameter_types)?,
                parameter_types_set_sha256: fingerprint.parameter_types_set_sha256.clone(),
                typed_shifted_bind_set_sha256: hash_serialized(&group_typed_bind_hashes)?,
            });
        }
        sqls.push(fingerprint);
    }

    connection
        .close()
        .await
        .map_err(|_| "database connection cleanup failed".to_string())?;
    sqls.sort_by(|left, right| left.sql_sha256.cmp(&right.sql_sha256));

    let statement_sqls: BTreeSet<_> = parsed.statements.keys().cloned().collect();
    let parse_excluded_sqls: BTreeSet<_> =
        parsed.excluded.iter().map(|row| row.sql.clone()).collect();
    let all_configured_exclusion_sql_present = exclusions.iter().all(|exclusion| {
        statement_sqls.contains(&exclusion.sql) || parse_excluded_sqls.contains(&exclusion.sql)
    });
    let applicable_exclusions =
        fingerprints_for_applicable_exclusions(exclusions, &statement_sqls, &parse_excluded_sqls);

    let configured_exclusions = configured_exclusions;
    let parser_exclusions = parser_exclusions;
    let preflight_exclusions = fingerprints_for_exclusion_map(&preflight_exclusions);
    let sql_hashes: Vec<_> = sqls.iter().map(|sql| sql.sql_sha256.as_str()).collect();

    let comparison = InputComparison {
        target_date: target_date.to_string(),
        parsed_sql_count: sqls.len(),
        unique_raw_bind_count: total_bind_count,
        sql_set_sha256: hash_serialized(&sql_hashes)?,
        sqls,
        configured_exclusion_count: configured_exclusions.len(),
        configured_exclusion_set_sha256: hash_serialized(&configured_exclusions)?,
        configured_exclusions,
        applicable_exclusion_count: applicable_exclusions.len(),
        applicable_exclusion_set_sha256: hash_serialized(&applicable_exclusions)?,
        applicable_exclusions,
        parser_exclusion_count: parser_exclusions.len(),
        parser_exclusion_set_sha256: hash_serialized(&parser_exclusions)?,
        parser_exclusions,
        preflight_exclusion_count: preflight_exclusions.len(),
        preflight_exclusion_set_sha256: hash_serialized(&preflight_exclusions)?,
        preflight_exclusions,
        all_configured_exclusion_sql_present,
        group_movers,
    };
    raw_group_binds.sort_by(|left, right| left.shifted_parameters.cmp(&right.shifted_parameters));

    Ok((comparison, raw_group_binds))
}

pub fn fingerprints_for_configured_exclusions(
    exclusions: &[Exclusion],
) -> Vec<ExclusionFingerprint> {
    let mut fingerprints: Vec<_> = exclusions
        .iter()
        .map(|exclusion| ExclusionFingerprint {
            sql_sha256: sha256(exclusion.sql.as_bytes()),
            reason_sha256: sha256(exclusion.reason.as_bytes()),
        })
        .collect();
    fingerprints.sort();
    fingerprints
}

fn fingerprints_for_applicable_exclusions(
    exclusions: &[Exclusion],
    statement_sqls: &BTreeSet<String>,
    parse_excluded_sqls: &BTreeSet<String>,
) -> Vec<ExclusionFingerprint> {
    let mut fingerprints: Vec<_> = exclusions
        .iter()
        .filter(|exclusion| {
            statement_sqls.contains(&exclusion.sql) || parse_excluded_sqls.contains(&exclusion.sql)
        })
        .map(|exclusion| ExclusionFingerprint {
            sql_sha256: sha256(exclusion.sql.as_bytes()),
            reason_sha256: sha256(exclusion.reason.as_bytes()),
        })
        .collect();
    fingerprints.sort();
    fingerprints
}

fn fingerprints_for_parsed_exclusions(
    exclusions: &[ExcludedStatement],
) -> Vec<ExclusionFingerprint> {
    let mut fingerprints: Vec<_> = exclusions
        .iter()
        .map(|exclusion| ExclusionFingerprint {
            sql_sha256: sha256(exclusion.sql.as_bytes()),
            reason_sha256: sha256(exclusion.reason.as_bytes()),
        })
        .collect();
    fingerprints.sort();
    fingerprints
}

fn fingerprints_for_exclusion_map(
    exclusions: &BTreeMap<String, String>,
) -> Vec<ExclusionFingerprint> {
    let mut fingerprints: Vec<_> = exclusions
        .iter()
        .map(|(sql, reason)| ExclusionFingerprint {
            sql_sha256: sha256(sql.as_bytes()),
            reason_sha256: sha256(reason.as_bytes()),
        })
        .collect();
    fingerprints.sort();
    fingerprints
}

fn bind_fingerprint(
    parameter_types: &[String],
    shifted_parameters: &[Option<String>],
) -> Result<BindFingerprint, String> {
    Ok(BindFingerprint {
        logged_parameter_count: shifted_parameters.len(),
        prepared_parameter_count: parameter_types.len(),
        parameter_types_sha256: hash_serialized(parameter_types)?,
        shifted_values_sha256: hash_serialized(shifted_parameters)?,
        typed_shifted_bind_sha256: hash_serialized(&(parameter_types, shifted_parameters))?,
    })
}

fn prepare_failure_detail(sql_sha256: &str, sqlstate: Option<&str>) -> String {
    let sqlstate = sqlstate
        .filter(|code| {
            code.len() == 5
                && code
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        })
        .unwrap_or("unavailable");

    format!(
        "parameter type resolution failed for SQL {sql_sha256}: phase=prepare_statement sqlstate={sqlstate}"
    )
}

#[cfg(test)]
mod tests {
    use super::prepare_failure_detail;

    #[test]
    fn prepare_failure_detail_keeps_only_phase_and_sqlstate() {
        let sql_sha256 = "a".repeat(64);

        assert_eq!(
            [
                prepare_failure_detail(&sql_sha256, Some("42P01")),
                prepare_failure_detail(&sql_sha256, Some("private error text")),
                prepare_failure_detail(&sql_sha256, None),
            ],
            [
                format!(
                    "parameter type resolution failed for SQL {sql_sha256}: phase=prepare_statement sqlstate=42P01"
                ),
                format!(
                    "parameter type resolution failed for SQL {sql_sha256}: phase=prepare_statement sqlstate=unavailable"
                ),
                format!(
                    "parameter type resolution failed for SQL {sql_sha256}: phase=prepare_statement sqlstate=unavailable"
                ),
            ],
        );
    }
}

fn is_group_movers_sql(sql: &str) -> bool {
    sql.contains("eligible_stocks AS (") && sql.contains("period_bars AS MATERIALIZED (")
}
