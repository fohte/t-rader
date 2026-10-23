use crate::{
    comparison::compare_manifests,
    model::{ComparisonReport, Manifest, Provenance, SqlDifference},
    runner::{ExclusionFingerprint, InputComparison, SqlFingerprint},
};

#[test]
fn identical_manifests_match() {
    let baseline = manifest(
        sql_fingerprint('b', 'c', 'd'),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(compare_manifests(&baseline, &baseline), expected_report());
}

#[test]
fn typed_bind_differences_fail_comparison() {
    let baseline_sql = sql_fingerprint('b', 'c', 'd');
    let baseline = manifest(
        baseline_sql.clone(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut changed_sql = baseline_sql;
    changed_sql.parameter_types_set_sha256 = digest('e');
    changed_sql.shifted_bind_set_sha256 = digest('f');
    changed_sql.typed_shifted_bind_set_sha256 = digest('g');
    let changed = manifest(changed_sql, Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut expected = expected_report();
    expected.matches = false;
    expected.bind_set_mismatch_sql_count = 1;
    expected.parameter_type_mismatch_sql_count = 1;
    expected.shifted_bind_mismatch_sql_count = 1;
    expected.sql_differences = vec![SqlDifference {
        sql_sha256: digest('a'),
        expected_present: true,
        actual_present: true,
        expected_bind_count: Some(1),
        actual_bind_count: Some(1),
        bind_set_matches: false,
        parameter_types_match: false,
        shifted_bind_values_match: false,
    }];

    assert_eq!(compare_manifests(&baseline, &changed), expected);
}

#[test]
fn all_exclusion_set_differences_fail_comparison() {
    let sql = sql_fingerprint('b', 'c', 'd');
    let baseline = manifest(sql.clone(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let exclusion = ExclusionFingerprint {
        sql_sha256: digest('h'),
        reason_sha256: digest('i'),
    };
    let changed = manifest(
        sql,
        vec![exclusion.clone()],
        vec![exclusion.clone()],
        vec![exclusion.clone()],
        vec![exclusion],
    );
    let mut expected = expected_report();
    expected.matches = false;
    expected.configured_exclusion_set_matches = false;
    expected.actual_configured_exclusion_count = 1;
    expected.configured_exclusion_difference_count = 1;
    expected.applicable_exclusion_set_matches = false;
    expected.actual_applicable_exclusion_count = 1;
    expected.applicable_exclusion_difference_count = 1;
    expected.parser_exclusion_set_matches = false;
    expected.actual_parser_exclusion_count = 1;
    expected.parser_exclusion_difference_count = 1;
    expected.preflight_exclusion_set_matches = false;
    expected.actual_preflight_exclusion_count = 1;
    expected.preflight_exclusion_difference_count = 1;

    assert_eq!(compare_manifests(&baseline, &changed), expected);
}

fn manifest(
    sql: SqlFingerprint,
    configured_exclusions: Vec<ExclusionFingerprint>,
    applicable_exclusions: Vec<ExclusionFingerprint>,
    parser_exclusions: Vec<ExclusionFingerprint>,
    preflight_exclusions: Vec<ExclusionFingerprint>,
) -> Manifest {
    let configured_exclusion_count = configured_exclusions.len();
    let applicable_exclusion_count = applicable_exclusions.len();
    let parser_exclusion_count = parser_exclusions.len();
    let preflight_exclusion_count = preflight_exclusions.len();
    Manifest {
        format_version: 3,
        provenance: Provenance {
            input_kind: "synthetic".to_string(),
            source_checkout_sha: "synthetic-source".to_string(),
            source_log_sha256: digest('j'),
            raw_record_count: 1,
            input_artifact_name: None,
            input_artifact_id: None,
            reference_source_run_id: "synthetic-run".to_string(),
            reference_source_artifact_name: "synthetic-artifact".to_string(),
            reference_source_artifact_id: "synthetic-id".to_string(),
            reference_source_zip_sha256: digest('k'),
            reference_source_zip_available: false,
            reference_source_zip_identity_proven: false,
            reference_local_log_sha256: digest('l'),
            reference_replay_sql_count: 1,
            reference_applied_exclusion_count: 0,
            reference_applied_exclusion_set_sha256: digest('m'),
        },
        comparison: InputComparison {
            target_date: "2099-01-01".to_string(),
            parsed_sql_count: 1,
            unique_raw_bind_count: 1,
            sql_set_sha256: digest('n'),
            sqls: vec![sql],
            configured_exclusion_count,
            configured_exclusion_set_sha256: digest('o'),
            configured_exclusions,
            applicable_exclusion_count,
            applicable_exclusion_set_sha256: digest('p'),
            applicable_exclusions,
            parser_exclusion_count,
            parser_exclusion_set_sha256: digest('q'),
            parser_exclusions,
            preflight_exclusion_count,
            preflight_exclusion_set_sha256: digest('r'),
            preflight_exclusions,
            all_configured_exclusion_sql_present: true,
            group_movers: None,
        },
    }
}

fn sql_fingerprint(
    parameter_types: char,
    shifted_values: char,
    typed_bind: char,
) -> SqlFingerprint {
    SqlFingerprint {
        sql_sha256: digest('a'),
        bind_count: 1,
        parameter_types_set_sha256: digest(parameter_types),
        shifted_bind_set_sha256: digest(shifted_values),
        typed_shifted_bind_set_sha256: digest(typed_bind),
    }
}

fn expected_report() -> ComparisonReport {
    ComparisonReport {
        matches: true,
        expected_source_log_sha256: digest('j'),
        actual_source_log_sha256: digest('j'),
        reference_replay_sql_count: 1,
        reference_applied_exclusion_count: 0,
        expected_parsed_sql_count: 1,
        actual_parsed_sql_count: 1,
        missing_sql_count: 0,
        added_sql_count: 0,
        bind_set_mismatch_sql_count: 0,
        parameter_type_mismatch_sql_count: 0,
        shifted_bind_mismatch_sql_count: 0,
        configured_exclusion_set_matches: true,
        expected_configured_exclusion_count: 0,
        actual_configured_exclusion_count: 0,
        configured_exclusion_difference_count: 0,
        applicable_exclusion_set_matches: true,
        expected_applicable_exclusion_count: 0,
        actual_applicable_exclusion_count: 0,
        applicable_exclusion_difference_count: 0,
        parser_exclusion_set_matches: true,
        expected_parser_exclusion_count: 0,
        actual_parser_exclusion_count: 0,
        parser_exclusion_difference_count: 0,
        preflight_exclusion_set_matches: true,
        expected_preflight_exclusion_count: 0,
        actual_preflight_exclusion_count: 0,
        preflight_exclusion_difference_count: 0,
        target_date_matches: true,
        group_movers_match: true,
        all_configured_exclusion_sql_present: true,
        sql_differences: Vec::new(),
    }
}

fn digest(character: char) -> String {
    std::iter::repeat_n(character, 64).collect()
}
