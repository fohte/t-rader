use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
    process::ExitCode,
};

use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use runner::InputComparison;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[path = "../../../crates/apps/query-perf/src/log.rs"]
mod query_log;

mod runner {
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
                        return Err(format!(
                            "parameter type resolution failed for SQL {sql_sha256}: {}",
                            execution::error_kind(&error)
                        ));
                    }
                };

                if parameter_types.len() != parameters.len() {
                    let _ = execution::deallocate_statement(&mut connection, &statement_name).await;
                    let exclusion = exclusions.iter().find(|entry| entry.sql == *sql);
                    if parameters.is_empty()
                        && let Some(exclusion) = exclusion
                    {
                        preflight_exclusions
                            .insert(exclusion.sql.clone(), exclusion.reason.clone());
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
                                execution::deallocate_statement(&mut connection, &statement_name)
                                    .await;
                            return Err(format!("parameter shifting failed for SQL {sql_sha256}"));
                        }
                    };

                if parameters::build_execute_sql(&statement_name, &shifted_parameters).is_err() {
                    let _ = execution::deallocate_statement(&mut connection, &statement_name).await;
                    let exclusion = exclusions.iter().find(|entry| entry.sql == *sql);
                    if let Some(exclusion) = exclusion {
                        preflight_exclusions
                            .insert(exclusion.sql.clone(), exclusion.reason.clone());
                        binds.push(bind_fingerprint(&parameter_types, &shifted_parameters)?);
                        continue;
                    }
                    return Err(format!("parameter encoding failed for SQL {sql_sha256}"));
                }

                execution::deallocate_statement(&mut connection, &statement_name)
                    .await
                    .map_err(|_| {
                        format!("prepared statement cleanup failed for SQL {sql_sha256}")
                    })?;

                let fingerprint = bind_fingerprint(&parameter_types, &shifted_parameters)?;
                if is_group_movers_sql(sql) {
                    if group_parameter_types
                        .as_ref()
                        .is_some_and(|expected| expected != &parameter_types)
                    {
                        return Err(
                            "LIST_GROUP_MOVERS_SQL parameter types changed between binds"
                                .to_string(),
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
                let expected_types =
                    ["text", "date", "date", "numeric", "bigint"].map(str::to_string);
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
        let applicable_exclusions = fingerprints_for_applicable_exclusions(
            exclusions,
            &statement_sqls,
            &parse_excluded_sqls,
        );

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
        raw_group_binds
            .sort_by(|left, right| left.shifted_parameters.cmp(&right.shifted_parameters));

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
                statement_sqls.contains(&exclusion.sql)
                    || parse_excluded_sqls.contains(&exclusion.sql)
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

    fn is_group_movers_sql(sql: &str) -> bool {
        sql.contains("eligible_stocks AS (") && sql.contains("period_bars AS MATERIALIZED (")
    }
}

#[derive(Debug, Parser)]
#[command(about = "Creates and compares hash-only query-perf input manifests")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build {
        #[arg(long)]
        log_file: PathBuf,
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        target_date: String,
        #[arg(long)]
        source_checkout_sha: String,
        #[arg(long, default_value = "ci-regenerated")]
        input_kind: String,
        #[arg(long)]
        expected_log_sha256: Option<String>,
        #[arg(long)]
        input_artifact_name: Option<String>,
        #[arg(long)]
        input_artifact_id: Option<String>,
        #[arg(long)]
        exclusions_file: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        group_binds_output: Option<PathBuf>,
        #[arg(long)]
        compare_to: Option<PathBuf>,
        #[arg(long)]
        comparison_output: Option<PathBuf>,
        #[arg(long)]
        failure_output: Option<PathBuf>,
    },
    VerifyReplayExclusions {
        #[arg(long)]
        stdout: PathBuf,
        #[arg(long)]
        exclusions_file: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}

struct BuildOptions {
    log_file: PathBuf,
    database_url: String,
    target_date: String,
    source_checkout_sha: String,
    input_kind: String,
    expected_log_sha256: Option<String>,
    input_artifact_name: Option<String>,
    input_artifact_id: Option<String>,
    exclusions_file: PathBuf,
    output: PathBuf,
    group_binds_output: Option<PathBuf>,
    compare_to: Option<PathBuf>,
    comparison_output: Option<PathBuf>,
    failure_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Provenance {
    input_kind: String,
    source_checkout_sha: String,
    source_log_sha256: String,
    raw_record_count: usize,
    input_artifact_name: Option<String>,
    input_artifact_id: Option<String>,
    reference_source_run_id: String,
    reference_source_artifact_name: String,
    reference_source_artifact_id: String,
    reference_source_zip_sha256: String,
    reference_source_zip_available: bool,
    reference_source_zip_identity_proven: bool,
    reference_local_log_sha256: String,
    reference_replay_sql_count: usize,
    reference_applied_exclusion_count: usize,
    reference_applied_exclusion_set_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Manifest {
    format_version: u32,
    provenance: Provenance,
    comparison: InputComparison,
}

#[derive(Clone, Debug, Serialize)]
struct ComparisonReport {
    matches: bool,
    expected_source_log_sha256: String,
    actual_source_log_sha256: String,
    reference_replay_sql_count: usize,
    reference_applied_exclusion_count: usize,
    expected_parsed_sql_count: usize,
    actual_parsed_sql_count: usize,
    missing_sql_count: usize,
    added_sql_count: usize,
    bind_set_mismatch_sql_count: usize,
    parameter_type_mismatch_sql_count: usize,
    shifted_bind_mismatch_sql_count: usize,
    configured_exclusion_set_matches: bool,
    expected_configured_exclusion_count: usize,
    actual_configured_exclusion_count: usize,
    configured_exclusion_difference_count: usize,
    applicable_exclusion_set_matches: bool,
    expected_applicable_exclusion_count: usize,
    actual_applicable_exclusion_count: usize,
    applicable_exclusion_difference_count: usize,
    parser_exclusion_set_matches: bool,
    expected_parser_exclusion_count: usize,
    actual_parser_exclusion_count: usize,
    parser_exclusion_difference_count: usize,
    preflight_exclusion_set_matches: bool,
    expected_preflight_exclusion_count: usize,
    actual_preflight_exclusion_count: usize,
    preflight_exclusion_difference_count: usize,
    target_date_matches: bool,
    group_movers_match: bool,
    all_configured_exclusion_sql_present: bool,
    sql_differences: Vec<SqlDifference>,
}

#[derive(Clone, Debug, Serialize)]
struct SqlDifference {
    sql_sha256: String,
    expected_present: bool,
    actual_present: bool,
    expected_bind_count: Option<usize>,
    actual_bind_count: Option<usize>,
    bind_set_matches: bool,
    parameter_types_match: bool,
    shifted_bind_values_match: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ReplayExclusionReport {
    matches_configured_set: bool,
    configured_count: usize,
    observed_count: usize,
    missing_count: usize,
    unexpected_count: usize,
    configured_set_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
struct FailureReport {
    status: &'static str,
    failure_kind: String,
    difference_count: usize,
    difference_counts: BTreeMap<String, usize>,
    detail: String,
    input_log_sha256: Option<String>,
    raw_record_count: Option<usize>,
}

const EXPECTED_SOURCE_SHA: &str = "d24a3e71dac02684f1d752c4d37aeb6462ca5c91";
const EXPECTED_LOCAL_LOG_SHA256: &str =
    "6a095a31d0d22eaf1517c23ae48b374056017ad7eabf26fd882ebf6b6af3320b";
const EXPECTED_SOURCE_RUN_ID: &str = "39714934810";
const EXPECTED_SOURCE_ARTIFACT_NAME: &str = "backend-query-log";
const EXPECTED_SOURCE_ARTIFACT_ID: &str = "12399056019";
const EXPECTED_SOURCE_ZIP_SHA256: &str =
    "50d7b38e3a07a9939cc87f9e1bdbf5084240c9ab13ecd53e1ddb34ad50b499e6";
const EXPECTED_REFERENCE_MANIFEST_SHA256: &str =
    "021b14429c4bdddba79594e45730508bd98656bf0412c638f3e7f6061da83fc5";
const EXPECTED_TARGET_DATE: &str = "2026-10-22";
const EXPECTED_PARSED_SQL_COUNT: usize = 365;
const EXPECTED_REPLAY_SQL_COUNT: usize = 350;
const EXPECTED_APPLIED_EXCLUSION_COUNT: usize = 15;
const EXPECTED_GROUP_BIND_COUNT: usize = 6;
const EXPECTED_SPECIFIED_GROUP_BIND_COUNT: usize = 4;
const EXPECTED_SPECIFIED_GROUP_BIND_HASHES: [&str; 4] = [
    "f543de43dfeae2a666967a1de960cc763b65182a121756facc2f8c281afd049a",
    "0393d57d16b2de85dba2b7ef413e2c027b9e5619d2600a258d68a12398e15bb2",
    "920f8ba84771e45490ae05c5c0aaec96a6e95524438e01b754a51955dc642365",
    "6396bc78b635a082acf0ab3e54ce51acf88d70a503e4c4a24bcb21548cccdb5c",
];

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let args = Args::parse();
    match args.command {
        Command::Build {
            log_file,
            database_url,
            target_date,
            source_checkout_sha,
            input_kind,
            expected_log_sha256,
            input_artifact_name,
            input_artifact_id,
            exclusions_file,
            output,
            group_binds_output,
            compare_to,
            comparison_output,
            failure_output,
        } => {
            build_manifest(BuildOptions {
                log_file,
                database_url,
                target_date,
                source_checkout_sha,
                input_kind,
                expected_log_sha256,
                input_artifact_name,
                input_artifact_id,
                exclusions_file,
                output,
                group_binds_output,
                compare_to,
                comparison_output,
                failure_output,
            })
            .await
        }
        Command::VerifyReplayExclusions {
            stdout,
            exclusions_file,
            output,
        } => verify_replay_exclusions(stdout, exclusions_file, output),
    }
}

async fn build_manifest(options: BuildOptions) -> Result<(), String> {
    let failure_output = options.failure_output.clone();
    let log_file = options.log_file.clone();
    let comparison_output = options.comparison_output.clone();
    let result = build_manifest_inner(options).await;
    if let Err(detail) = &result
        && let Some(failure_output) = failure_output
    {
        let contents = fs::read(&log_file).ok();
        let text = contents
            .as_deref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok());
        let failure_kind = failure_kind(detail);
        let mut difference_counts = BTreeMap::new();
        if let Some(comparison_output) = comparison_output
            && let Ok(comparison_bytes) = fs::read(comparison_output)
            && let Ok(comparison) = serde_json::from_slice::<serde_json::Value>(&comparison_bytes)
        {
            for field in [
                "missing_sql_count",
                "added_sql_count",
                "bind_set_mismatch_sql_count",
                "parameter_type_mismatch_sql_count",
                "shifted_bind_mismatch_sql_count",
                "configured_exclusion_difference_count",
                "applicable_exclusion_difference_count",
                "parser_exclusion_difference_count",
                "preflight_exclusion_difference_count",
            ] {
                if let Some(count) = comparison.get(field).and_then(serde_json::Value::as_u64)
                    && count > 0
                {
                    difference_counts.insert(field.to_string(), count as usize);
                }
            }
            for (field, difference_name) in [
                ("target_date_matches", "target_date_mismatch_count"),
                ("group_movers_match", "group_movers_mismatch_count"),
                (
                    "all_configured_exclusion_sql_present",
                    "missing_configured_exclusion_sql_count",
                ),
            ] {
                if comparison.get(field).and_then(serde_json::Value::as_bool) == Some(false) {
                    difference_counts.insert(difference_name.to_string(), 1);
                }
            }
        }
        if difference_counts.is_empty() {
            difference_counts.insert(failure_kind.clone(), 1);
        }
        let difference_count = difference_counts.values().sum();
        let report = FailureReport {
            status: "failed",
            failure_kind: failure_kind.clone(),
            difference_count,
            difference_counts,
            detail: detail.clone(),
            input_log_sha256: contents.as_deref().map(sha256),
            raw_record_count: text
                .map(|text| text.lines().filter(|line| !line.trim().is_empty()).count()),
        };
        write_json(&failure_output, &report)?;
    }
    result
}

async fn build_manifest_inner(options: BuildOptions) -> Result<(), String> {
    let BuildOptions {
        log_file,
        database_url,
        target_date,
        source_checkout_sha,
        input_kind,
        expected_log_sha256,
        input_artifact_name,
        input_artifact_id,
        exclusions_file,
        output,
        group_binds_output,
        compare_to,
        comparison_output,
        failure_output: _,
    } = options;
    let target_date_text = target_date.as_str();
    if source_checkout_sha != EXPECTED_SOURCE_SHA {
        return Err("source checkout SHA does not match the fixed source".to_string());
    }
    if target_date_text != EXPECTED_TARGET_DATE {
        return Err("target date does not match the fixed replay target".to_string());
    }
    match input_kind.as_str() {
        "unverified-local-reference" => {
            if compare_to.is_some() || comparison_output.is_some() {
                return Err(
                    "local reference generation cannot compare another manifest".to_string()
                );
            }
        }
        "ci-regenerated" => {
            if compare_to.is_none() || comparison_output.is_none() {
                return Err("CI-regenerated input requires a full baseline comparison".to_string());
            }
            if input_artifact_name.is_none() || input_artifact_id.is_none() {
                return Err(
                    "CI-regenerated input requires its Actions artifact name and ID".to_string(),
                );
            }
            if group_binds_output.is_none() {
                return Err("CI-regenerated input requires a group bind output path".to_string());
            }
        }
        _ => return Err("input kind is not recognized".to_string()),
    }
    if comparison_output.is_some() && compare_to.is_none() {
        return Err("comparison output was supplied without a baseline manifest".to_string());
    }

    let contents = fs::read(&log_file).map_err(|_| "query log could not be read".to_string())?;
    let source_log_sha256 = sha256(&contents);
    if expected_log_sha256
        .as_deref()
        .is_some_and(|expected| expected != source_log_sha256)
    {
        return Err("query log SHA-256 does not match the supplied reference".to_string());
    }
    if input_kind == "unverified-local-reference" && source_log_sha256 != EXPECTED_LOCAL_LOG_SHA256
    {
        return Err(
            "local reference query log SHA-256 differs from its recorded provenance".to_string(),
        );
    }
    let text = String::from_utf8(contents.clone())
        .map_err(|_| "query log is not valid UTF-8".to_string())?;
    let raw_record_count = text.lines().filter(|line| !line.trim().is_empty()).count();
    let exclusions_bytes = fs::read(&exclusions_file)
        .map_err(|_| "query-perf exclusions could not be read".to_string())?;
    let exclusions: Vec<query_log::Exclusion> = serde_json::from_slice(&exclusions_bytes)
        .map_err(|_| "query-perf exclusions are not valid JSON".to_string())?;
    if exclusions
        .iter()
        .any(|entry| entry.sql.trim().is_empty() || entry.reason.trim().is_empty())
    {
        return Err("query-perf exclusions contain an empty SQL or reason".to_string());
    }
    let parsed = query_log::parse_json_log(&text, &exclusions)
        .map_err(|_| "query log parsing failed".to_string())?;
    if input_kind == "unverified-local-reference"
        && parsed.statements.len() != EXPECTED_PARSED_SQL_COUNT
    {
        return Err(format!(
            "parsed SQL count is {}, expected {EXPECTED_PARSED_SQL_COUNT}",
            parsed.statements.len()
        ));
    }
    let target_date = NaiveDate::parse_from_str(target_date_text, "%Y-%m-%d")
        .map_err(|_| "target date is invalid".to_string())?;
    let (comparison, raw_group_binds) =
        runner::fingerprint(parsed, &exclusions, &database_url, target_date).await?;
    if input_kind == "unverified-local-reference"
        && comparison.parsed_sql_count != EXPECTED_PARSED_SQL_COUNT
    {
        return Err(format!(
            "parsed SQL count is {}, expected {EXPECTED_PARSED_SQL_COUNT}",
            comparison.parsed_sql_count
        ));
    }
    if input_kind == "unverified-local-reference"
        && comparison.group_movers.as_ref().is_none_or(|group_movers| {
            group_movers.bind_count != EXPECTED_GROUP_BIND_COUNT
                || group_movers.specified_bind_count != EXPECTED_SPECIFIED_GROUP_BIND_COUNT
        })
    {
        let group_movers = comparison.group_movers.as_ref();
        let bind_count = group_movers.map_or(0, |group_movers| group_movers.bind_count);
        let specified_bind_count =
            group_movers.map_or(0, |group_movers| group_movers.specified_bind_count);
        return Err(format!(
            "LIST_GROUP_MOVERS_SQL bind count is {}, specified bind count is {}; expected {EXPECTED_GROUP_BIND_COUNT} and {EXPECTED_SPECIFIED_GROUP_BIND_COUNT}",
            bind_count, specified_bind_count
        ));
    }
    if let Some(reason) = manifest_internal_validation_error(&comparison) {
        return Err(format!("generated manifest validation failed: {reason}"));
    }
    if input_kind == "unverified-local-reference"
        && raw_group_binds.len() != EXPECTED_GROUP_BIND_COUNT
    {
        return Err(format!(
            "LIST_GROUP_MOVERS_SQL has {} raw binds, expected {EXPECTED_GROUP_BIND_COUNT}",
            raw_group_binds.len()
        ));
    }
    let reference_applied_exclusion_set_sha256 = comparison.applicable_exclusion_set_sha256.clone();
    let manifest = Manifest {
        format_version: 3,
        provenance: Provenance {
            input_kind,
            source_checkout_sha,
            source_log_sha256,
            raw_record_count,
            input_artifact_name,
            input_artifact_id,
            reference_source_run_id: EXPECTED_SOURCE_RUN_ID.to_string(),
            reference_source_artifact_name: EXPECTED_SOURCE_ARTIFACT_NAME.to_string(),
            reference_source_artifact_id: EXPECTED_SOURCE_ARTIFACT_ID.to_string(),
            reference_source_zip_sha256: EXPECTED_SOURCE_ZIP_SHA256.to_string(),
            reference_source_zip_available: false,
            reference_source_zip_identity_proven: false,
            reference_local_log_sha256: EXPECTED_LOCAL_LOG_SHA256.to_string(),
            reference_replay_sql_count: EXPECTED_REPLAY_SQL_COUNT,
            reference_applied_exclusion_count: EXPECTED_APPLIED_EXCLUSION_COUNT,
            reference_applied_exclusion_set_sha256,
        },
        comparison,
    };
    write_json(&output, &manifest)?;

    if let Some(baseline_path) = compare_to {
        let comparison_output =
            comparison_output.ok_or_else(|| "comparison output path is required".to_string())?;
        let baseline_bytes = fs::read(baseline_path)
            .map_err(|_| "baseline manifest could not be read".to_string())?;
        if sha256(&baseline_bytes) != EXPECTED_REFERENCE_MANIFEST_SHA256 {
            return Err(
                "hash-only reference manifest SHA-256 is not the reviewed value".to_string(),
            );
        }
        let baseline: Manifest = serde_json::from_slice(&baseline_bytes)
            .map_err(|_| "baseline manifest is not valid JSON".to_string())?;
        validate_baseline(&baseline)?;
        let report = compare_manifests(&baseline, &manifest);
        write_json(&comparison_output, &report)?;
        if !report.matches {
            return Err(
                "regenerated query log differs from the hash-only reference manifest".to_string(),
            );
        }
    }
    if let Some(group_binds_output) = group_binds_output {
        write_json(&group_binds_output, &raw_group_binds)?;
    }
    Ok(())
}

fn validate_baseline(baseline: &Manifest) -> Result<(), String> {
    if baseline.provenance.input_kind != "unverified-local-reference"
        || baseline.provenance.source_checkout_sha != EXPECTED_SOURCE_SHA
        || baseline.provenance.source_log_sha256 != EXPECTED_LOCAL_LOG_SHA256
        || baseline.provenance.reference_source_run_id != EXPECTED_SOURCE_RUN_ID
        || baseline.provenance.reference_source_artifact_name != EXPECTED_SOURCE_ARTIFACT_NAME
        || baseline.provenance.reference_source_artifact_id != EXPECTED_SOURCE_ARTIFACT_ID
        || baseline.provenance.reference_source_zip_sha256 != EXPECTED_SOURCE_ZIP_SHA256
        || baseline.provenance.reference_source_zip_available
        || baseline.provenance.reference_source_zip_identity_proven
        || baseline.provenance.reference_local_log_sha256 != EXPECTED_LOCAL_LOG_SHA256
        || baseline.provenance.reference_replay_sql_count != EXPECTED_REPLAY_SQL_COUNT
        || baseline.provenance.reference_applied_exclusion_count != EXPECTED_APPLIED_EXCLUSION_COUNT
        || baseline.provenance.reference_applied_exclusion_set_sha256
            != baseline.comparison.applicable_exclusion_set_sha256
        || baseline.provenance.raw_record_count != 18_938
        || baseline.comparison.target_date != EXPECTED_TARGET_DATE
        || baseline.comparison.parsed_sql_count != EXPECTED_PARSED_SQL_COUNT
        || baseline.comparison.configured_exclusion_count != EXPECTED_APPLIED_EXCLUSION_COUNT
        || baseline.comparison.applicable_exclusion_count != EXPECTED_APPLIED_EXCLUSION_COUNT
        || baseline
            .comparison
            .group_movers
            .as_ref()
            .is_none_or(|group_movers| {
                group_movers.bind_count != EXPECTED_GROUP_BIND_COUNT
                    || group_movers.specified_bind_count != EXPECTED_SPECIFIED_GROUP_BIND_COUNT
                    || EXPECTED_SPECIFIED_GROUP_BIND_HASHES.iter().any(|expected| {
                        !group_movers
                            .specified_bind_hashes
                            .iter()
                            .any(|actual| actual == expected)
                    })
            })
        || baseline.format_version != 3
        || manifest_internal_validation_error(&baseline.comparison).is_some()
    {
        return Err("hash-only reference manifest provenance or counts are invalid".to_string());
    }
    Ok(())
}

fn manifest_internal_validation_error(comparison: &InputComparison) -> Option<String> {
    let mut sqls = comparison.sqls.clone();
    sqls.sort_by(|left, right| left.sql_sha256.cmp(&right.sql_sha256));
    if sqls != comparison.sqls
        || sqls.len() != comparison.parsed_sql_count
        || sqls
            .windows(2)
            .any(|pair| pair[0].sql_sha256 == pair[1].sql_sha256)
    {
        return Some("parsed SQL rows are not sorted and unique or SQL count differs".to_string());
    }

    let mut sql_hashes: Vec<_> = comparison
        .sqls
        .iter()
        .map(|sql| sql.sql_sha256.as_str())
        .collect();
    sql_hashes.sort_unstable();
    if hash_serialized(&sql_hashes).as_deref() != Ok(comparison.sql_set_sha256.as_str()) {
        return Some("SQL set digest does not match its rows".to_string());
    }

    let mut total_bind_count = 0;
    for sql in &comparison.sqls {
        total_bind_count += sql.bind_count;
        if sql.bind_count == 0
            || !is_sha256(&sql.parameter_types_set_sha256)
            || !is_sha256(&sql.shifted_bind_set_sha256)
            || !is_sha256(&sql.typed_shifted_bind_set_sha256)
        {
            return Some(format!(
                "SQL {} bind fingerprint digest or count is invalid",
                sql.sql_sha256
            ));
        }
    }
    if total_bind_count != comparison.unique_raw_bind_count {
        return Some("total unique raw bind count does not match".to_string());
    }

    let exclusion_sets = [
        (
            comparison.configured_exclusion_count,
            &comparison.configured_exclusions,
            comparison.configured_exclusion_set_sha256.as_str(),
        ),
        (
            comparison.applicable_exclusion_count,
            &comparison.applicable_exclusions,
            comparison.applicable_exclusion_set_sha256.as_str(),
        ),
        (
            comparison.parser_exclusion_count,
            &comparison.parser_exclusions,
            comparison.parser_exclusion_set_sha256.as_str(),
        ),
        (
            comparison.preflight_exclusion_count,
            &comparison.preflight_exclusions,
            comparison.preflight_exclusion_set_sha256.as_str(),
        ),
    ];
    if exclusion_sets.iter().any(|(count, rows, expected_hash)| {
        *count != rows.len()
            || rows.windows(2).any(|pair| pair[0] > pair[1])
            || hash_serialized(rows).as_deref() != Ok(*expected_hash)
    }) {
        return Some(
            "exclusion rows are not sorted or an exclusion count or digest differs".to_string(),
        );
    }

    let Some(group_movers) = comparison.group_movers.as_ref() else {
        return None;
    };
    let Some(group_sql) = comparison
        .sqls
        .iter()
        .find(|sql| sql.sql_sha256 == group_movers.sql_sha256)
    else {
        return Some("LIST_GROUP_MOVERS_SQL fingerprint is missing".to_string());
    };
    let specified_bind_hashes = &group_movers.specified_bind_hashes;
    if !(group_sql.bind_count == group_movers.bind_count
        && group_sql.parameter_types_set_sha256 == group_movers.parameter_types_set_sha256
        && group_sql.typed_shifted_bind_set_sha256 == group_movers.typed_shifted_bind_set_sha256
        && specified_bind_hashes.len() == group_movers.specified_bind_count
        && specified_bind_hashes
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && specified_bind_hashes.iter().all(|hash| is_sha256(hash))
        && hash_serialized(specified_bind_hashes).as_deref()
            == Ok(group_movers.specified_bind_set_sha256.as_str())
        && is_sha256(&group_movers.parameter_types_sha256))
    {
        return Some("LIST_GROUP_MOVERS_SQL fingerprint counts or digest differ".to_string());
    }
    None
}

fn compare_manifests(expected: &Manifest, actual: &Manifest) -> ComparisonReport {
    let expected_sqls: BTreeMap<_, _> = expected
        .comparison
        .sqls
        .iter()
        .map(|sql| (sql.sql_sha256.as_str(), sql))
        .collect();
    let actual_sqls: BTreeMap<_, _> = actual
        .comparison
        .sqls
        .iter()
        .map(|sql| (sql.sql_sha256.as_str(), sql))
        .collect();
    let all_sql_hashes: BTreeSet<_> = expected_sqls
        .keys()
        .chain(actual_sqls.keys())
        .copied()
        .collect();
    let mut missing_sql_count = 0;
    let mut added_sql_count = 0;
    let mut bind_set_mismatch_sql_count = 0;
    let mut parameter_type_mismatch_sql_count = 0;
    let mut shifted_bind_mismatch_sql_count = 0;
    let mut sql_differences = Vec::new();

    for sql_sha256 in all_sql_hashes {
        let expected_sql = expected_sqls.get(sql_sha256).copied();
        let actual_sql = actual_sqls.get(sql_sha256).copied();
        match (expected_sql, actual_sql) {
            (None, Some(actual_sql)) => {
                added_sql_count += 1;
                sql_differences.push(SqlDifference {
                    sql_sha256: sql_sha256.to_string(),
                    expected_present: false,
                    actual_present: true,
                    expected_bind_count: None,
                    actual_bind_count: Some(actual_sql.bind_count),
                    bind_set_matches: false,
                    parameter_types_match: false,
                    shifted_bind_values_match: false,
                });
            }
            (Some(expected_sql), None) => {
                missing_sql_count += 1;
                sql_differences.push(SqlDifference {
                    sql_sha256: sql_sha256.to_string(),
                    expected_present: true,
                    actual_present: false,
                    expected_bind_count: Some(expected_sql.bind_count),
                    actual_bind_count: None,
                    bind_set_matches: false,
                    parameter_types_match: false,
                    shifted_bind_values_match: false,
                });
            }
            (Some(expected_sql), Some(actual_sql)) => {
                let bind_set_matches = expected_sql.bind_count == actual_sql.bind_count
                    && expected_sql.typed_shifted_bind_set_sha256
                        == actual_sql.typed_shifted_bind_set_sha256;
                let parameter_types_match = expected_sql.parameter_types_set_sha256
                    == actual_sql.parameter_types_set_sha256;
                let shifted_bind_values_match =
                    expected_sql.shifted_bind_set_sha256 == actual_sql.shifted_bind_set_sha256;
                if !bind_set_matches {
                    bind_set_mismatch_sql_count += 1;
                }
                if !parameter_types_match {
                    parameter_type_mismatch_sql_count += 1;
                }
                if !shifted_bind_values_match {
                    shifted_bind_mismatch_sql_count += 1;
                }
                if !bind_set_matches
                    || expected_sql.bind_count != actual_sql.bind_count
                    || !parameter_types_match
                    || !shifted_bind_values_match
                {
                    sql_differences.push(SqlDifference {
                        sql_sha256: sql_sha256.to_string(),
                        expected_present: true,
                        actual_present: true,
                        expected_bind_count: Some(expected_sql.bind_count),
                        actual_bind_count: Some(actual_sql.bind_count),
                        bind_set_matches,
                        parameter_types_match,
                        shifted_bind_values_match,
                    });
                }
            }
            (None, None) => {}
        }
    }
    let configured_exclusion_set_matches =
        expected.comparison.configured_exclusions == actual.comparison.configured_exclusions;
    let applicable_exclusion_set_matches =
        expected.comparison.applicable_exclusions == actual.comparison.applicable_exclusions;
    let parser_exclusion_set_matches =
        expected.comparison.parser_exclusions == actual.comparison.parser_exclusions;
    let preflight_exclusion_set_matches =
        expected.comparison.preflight_exclusions == actual.comparison.preflight_exclusions;
    let configured_exclusion_difference_count = symmetric_difference_count(
        &expected.comparison.configured_exclusions,
        &actual.comparison.configured_exclusions,
    );
    let applicable_exclusion_difference_count = symmetric_difference_count(
        &expected.comparison.applicable_exclusions,
        &actual.comparison.applicable_exclusions,
    );
    let parser_exclusion_difference_count = symmetric_difference_count(
        &expected.comparison.parser_exclusions,
        &actual.comparison.parser_exclusions,
    );
    let preflight_exclusion_difference_count = symmetric_difference_count(
        &expected.comparison.preflight_exclusions,
        &actual.comparison.preflight_exclusions,
    );
    let target_date_matches = expected.comparison.target_date == actual.comparison.target_date;
    let group_movers_match = expected.comparison.group_movers == actual.comparison.group_movers;
    let sql_differences_match = expected.comparison.parsed_sql_count
        == actual.comparison.parsed_sql_count
        && expected.comparison.unique_raw_bind_count == actual.comparison.unique_raw_bind_count
        && expected.comparison.sql_set_sha256 == actual.comparison.sql_set_sha256
        && missing_sql_count == 0
        && added_sql_count == 0
        && bind_set_mismatch_sql_count == 0
        && parameter_type_mismatch_sql_count == 0
        && shifted_bind_mismatch_sql_count == 0;
    let all_configured_exclusion_sql_present =
        expected.comparison.all_configured_exclusion_sql_present
            && actual.comparison.all_configured_exclusion_sql_present;
    let matches = expected.format_version == actual.format_version
        && sql_differences_match
        && configured_exclusion_set_matches
        && applicable_exclusion_set_matches
        && parser_exclusion_set_matches
        && preflight_exclusion_set_matches
        && target_date_matches
        && group_movers_match
        && all_configured_exclusion_sql_present;

    ComparisonReport {
        matches,
        expected_source_log_sha256: expected.provenance.source_log_sha256.clone(),
        actual_source_log_sha256: actual.provenance.source_log_sha256.clone(),
        reference_replay_sql_count: expected.provenance.reference_replay_sql_count,
        reference_applied_exclusion_count: expected.provenance.reference_applied_exclusion_count,
        expected_parsed_sql_count: expected.comparison.parsed_sql_count,
        actual_parsed_sql_count: actual.comparison.parsed_sql_count,
        missing_sql_count,
        added_sql_count,
        bind_set_mismatch_sql_count,
        parameter_type_mismatch_sql_count,
        shifted_bind_mismatch_sql_count,
        configured_exclusion_set_matches,
        expected_configured_exclusion_count: expected.comparison.configured_exclusion_count,
        actual_configured_exclusion_count: actual.comparison.configured_exclusion_count,
        configured_exclusion_difference_count,
        applicable_exclusion_set_matches,
        expected_applicable_exclusion_count: expected.comparison.applicable_exclusion_count,
        actual_applicable_exclusion_count: actual.comparison.applicable_exclusion_count,
        applicable_exclusion_difference_count,
        parser_exclusion_set_matches,
        expected_parser_exclusion_count: expected.comparison.parser_exclusion_count,
        actual_parser_exclusion_count: actual.comparison.parser_exclusion_count,
        parser_exclusion_difference_count,
        preflight_exclusion_set_matches,
        expected_preflight_exclusion_count: expected.comparison.preflight_exclusion_count,
        actual_preflight_exclusion_count: actual.comparison.preflight_exclusion_count,
        preflight_exclusion_difference_count,
        target_date_matches,
        group_movers_match,
        all_configured_exclusion_sql_present,
        sql_differences,
    }
}

fn symmetric_difference_count<T: Ord + Clone>(expected: &[T], actual: &[T]) -> usize {
    let expected: BTreeSet<_> = expected.iter().cloned().collect();
    let actual: BTreeSet<_> = actual.iter().cloned().collect();
    expected.symmetric_difference(&actual).count()
}

fn failure_kind(detail: &str) -> String {
    if detail.contains("query log parsing failed") {
        "parse_failure".to_string()
    } else if detail.contains("parsed SQL count") {
        "sql_count_mismatch".to_string()
    } else if detail.contains("parameter type") || detail.contains("PREPARE") {
        "parameter_type_resolution_failure".to_string()
    } else if detail.contains("exclusion") {
        "exclusion_mismatch".to_string()
    } else if detail.contains("differs") || detail.contains("manifest") {
        "input_comparison_failure".to_string()
    } else if detail.contains("target date") {
        "target_date_mismatch".to_string()
    } else {
        "manifest_preflight_failure".to_string()
    }
}

fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn verify_replay_exclusions(
    stdout_path: PathBuf,
    exclusions_path: PathBuf,
    output: PathBuf,
) -> Result<(), String> {
    let stdout = fs::read_to_string(stdout_path)
        .map_err(|_| "query-perf replay stdout could not be read".to_string())?;
    let exclusions_bytes = fs::read(exclusions_path)
        .map_err(|_| "query-perf exclusions could not be read".to_string())?;
    let exclusions: Vec<query_log::Exclusion> = serde_json::from_slice(&exclusions_bytes)
        .map_err(|_| "query-perf exclusions are not valid JSON".to_string())?;
    let observed_count = stdout.matches("EXCLUDED reason=").count();
    let missing_count = exclusions
        .iter()
        .filter(|exclusion| {
            !stdout.contains(&format!(
                "EXCLUDED reason={} SQL={}",
                exclusion.reason, exclusion.sql
            ))
        })
        .count();
    let configured_fingerprints = runner::fingerprints_for_configured_exclusions(&exclusions);
    let unexpected_count = observed_count.saturating_sub(exclusions.len());
    let report = ReplayExclusionReport {
        matches_configured_set: missing_count == 0 && observed_count == exclusions.len(),
        configured_count: exclusions.len(),
        observed_count,
        missing_count,
        unexpected_count,
        configured_set_sha256: hash_serialized(&configured_fingerprints)?,
    };
    write_json(&output, &report)?;
    if !report.matches_configured_set {
        return Err("query-perf applied exclusion set differs from the fixed source".to_string());
    }
    Ok(())
}

fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| "manifest output directory could not be created".to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| "manifest output could not be serialized".to_string())?;
    fs::write(path, bytes).map_err(|_| "manifest output could not be written".to_string())
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_serialized<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| "canonical hash serialization failed".to_string())?;
    Ok(sha256(&bytes))
}
