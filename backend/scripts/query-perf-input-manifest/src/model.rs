use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::runner::InputComparison;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Provenance {
    pub(crate) input_kind: String,
    pub(crate) source_checkout_sha: String,
    pub(crate) source_log_sha256: String,
    pub(crate) raw_record_count: usize,
    pub(crate) input_artifact_name: Option<String>,
    pub(crate) input_artifact_id: Option<String>,
    pub(crate) reference_source_run_id: String,
    pub(crate) reference_source_artifact_name: String,
    pub(crate) reference_source_artifact_id: String,
    pub(crate) reference_source_zip_sha256: String,
    pub(crate) reference_source_zip_available: bool,
    pub(crate) reference_source_zip_identity_proven: bool,
    pub(crate) reference_local_log_sha256: String,
    pub(crate) reference_replay_sql_count: usize,
    pub(crate) reference_applied_exclusion_count: usize,
    pub(crate) reference_applied_exclusion_set_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Manifest {
    pub(crate) format_version: u32,
    pub(crate) provenance: Provenance,
    pub(crate) comparison: InputComparison,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ComparisonReport {
    pub(crate) matches: bool,
    pub(crate) expected_source_log_sha256: String,
    pub(crate) actual_source_log_sha256: String,
    pub(crate) reference_replay_sql_count: usize,
    pub(crate) reference_applied_exclusion_count: usize,
    pub(crate) expected_parsed_sql_count: usize,
    pub(crate) actual_parsed_sql_count: usize,
    pub(crate) missing_sql_count: usize,
    pub(crate) added_sql_count: usize,
    pub(crate) bind_set_mismatch_sql_count: usize,
    pub(crate) parameter_type_mismatch_sql_count: usize,
    pub(crate) shifted_bind_mismatch_sql_count: usize,
    pub(crate) configured_exclusion_set_matches: bool,
    pub(crate) expected_configured_exclusion_count: usize,
    pub(crate) actual_configured_exclusion_count: usize,
    pub(crate) configured_exclusion_difference_count: usize,
    pub(crate) applicable_exclusion_set_matches: bool,
    pub(crate) expected_applicable_exclusion_count: usize,
    pub(crate) actual_applicable_exclusion_count: usize,
    pub(crate) applicable_exclusion_difference_count: usize,
    pub(crate) parser_exclusion_set_matches: bool,
    pub(crate) expected_parser_exclusion_count: usize,
    pub(crate) actual_parser_exclusion_count: usize,
    pub(crate) parser_exclusion_difference_count: usize,
    pub(crate) preflight_exclusion_set_matches: bool,
    pub(crate) expected_preflight_exclusion_count: usize,
    pub(crate) actual_preflight_exclusion_count: usize,
    pub(crate) preflight_exclusion_difference_count: usize,
    pub(crate) target_date_matches: bool,
    pub(crate) group_movers_match: bool,
    pub(crate) all_configured_exclusion_sql_present: bool,
    pub(crate) sql_differences: Vec<SqlDifference>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SqlDifference {
    pub(crate) sql_sha256: String,
    pub(crate) expected_present: bool,
    pub(crate) actual_present: bool,
    pub(crate) expected_bind_count: Option<usize>,
    pub(crate) actual_bind_count: Option<usize>,
    pub(crate) bind_set_matches: bool,
    pub(crate) parameter_types_match: bool,
    pub(crate) shifted_bind_values_match: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ReplayExclusionReport {
    pub(crate) matches_configured_set: bool,
    pub(crate) configured_count: usize,
    pub(crate) observed_count: usize,
    pub(crate) missing_count: usize,
    pub(crate) unexpected_count: usize,
    pub(crate) configured_set_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FailureReport {
    pub(crate) status: &'static str,
    pub(crate) failure_kind: String,
    pub(crate) difference_count: usize,
    pub(crate) difference_counts: BTreeMap<String, usize>,
    pub(crate) detail: String,
    pub(crate) input_log_sha256: Option<String>,
    pub(crate) raw_record_count: Option<usize>,
}
