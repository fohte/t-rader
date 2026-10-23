use std::{collections::BTreeMap, fs};

use chrono::NaiveDate;

use crate::{
    EXPECTED_APPLIED_EXCLUSION_COUNT, EXPECTED_GROUP_BIND_COUNT, EXPECTED_LOCAL_LOG_SHA256,
    EXPECTED_PARSED_SQL_COUNT, EXPECTED_REFERENCE_MANIFEST_SHA256, EXPECTED_REPLAY_SQL_COUNT,
    EXPECTED_SOURCE_ARTIFACT_ID, EXPECTED_SOURCE_ARTIFACT_NAME, EXPECTED_SOURCE_RUN_ID,
    EXPECTED_SOURCE_SHA, EXPECTED_SOURCE_ZIP_SHA256, EXPECTED_SPECIFIED_GROUP_BIND_COUNT,
    EXPECTED_TARGET_DATE,
    cli::BuildOptions,
    comparison::{
        compare_manifests, failure_kind, manifest_internal_validation_error, validate_baseline,
    },
    model::{FailureReport, Manifest, Provenance},
    query_log, runner, sha256, write_json,
};

pub(crate) async fn build_manifest(options: BuildOptions) -> Result<(), String> {
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
