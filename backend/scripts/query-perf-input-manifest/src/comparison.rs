use std::collections::{BTreeMap, BTreeSet};

use crate::{
    EXPECTED_APPLIED_EXCLUSION_COUNT, EXPECTED_GROUP_BIND_COUNT, EXPECTED_LOCAL_LOG_SHA256,
    EXPECTED_PARSED_SQL_COUNT, EXPECTED_REPLAY_SQL_COUNT, EXPECTED_SOURCE_ARTIFACT_ID,
    EXPECTED_SOURCE_ARTIFACT_NAME, EXPECTED_SOURCE_RUN_ID, EXPECTED_SOURCE_SHA,
    EXPECTED_SOURCE_ZIP_SHA256, EXPECTED_SPECIFIED_GROUP_BIND_COUNT,
    EXPECTED_SPECIFIED_GROUP_BIND_HASHES, EXPECTED_TARGET_DATE,
    model::{ComparisonReport, Manifest, SqlDifference},
    runner::InputComparison,
    util::hash_serialized,
};

pub(crate) fn validate_baseline(baseline: &Manifest) -> Result<(), String> {
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

pub(crate) fn manifest_internal_validation_error(comparison: &InputComparison) -> Option<String> {
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

pub(crate) fn compare_manifests(expected: &Manifest, actual: &Manifest) -> ComparisonReport {
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

pub(crate) fn failure_kind(detail: &str) -> String {
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
