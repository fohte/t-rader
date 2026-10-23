use std::{fs, path::PathBuf};

use crate::{
    model::ReplayExclusionReport,
    query_log, runner,
    util::{hash_serialized, write_json},
};

pub(crate) fn verify_replay_exclusions(
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
