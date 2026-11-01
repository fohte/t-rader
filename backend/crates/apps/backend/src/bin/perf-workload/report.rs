use std::{collections::BTreeMap, fs, path::Path};

use serde::Serialize;

#[derive(Serialize)]
pub(super) struct WorkloadReport {
    pub(super) required_case_ids: Vec<String>,
    pub(super) completed_case_ids: Vec<String>,
    pub(super) expected_sql_by_case: BTreeMap<String, Vec<String>>,
}

pub(super) fn write_report(path: &Path, report: &WorkloadReport) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(report)
        .map_err(|_| "could not serialize the workload report".to_string())?;
    fs::write(path, contents).map_err(|_| "could not write the workload report".to_string())
}
