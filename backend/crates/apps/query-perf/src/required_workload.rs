use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const APPLICATION_NAME_PREFIX: &str = "perf:";

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkloadReport {
    pub(super) required_case_ids: Vec<String>,
    pub(super) completed_case_ids: Vec<String>,
    pub(super) expected_sql_by_case: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct WorkloadVerification {
    pub(super) case_count: usize,
    pub(super) observed_sql_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WorkloadFailureKind {
    InvalidReport,
    InvalidSqlLog,
    DuplicateCaseId,
    MissingCase,
    UnexpectedCompletedCase,
    MissingExpectation,
    UnexpectedExpectation,
    DuplicateExpectation,
    MissingObservation,
    UnexpectedObservedCase,
    ExcludedOnly,
}

pub(super) fn parse_workload_report(contents: &str) -> Result<WorkloadReport, WorkloadFailureKind> {
    serde_json::from_str(contents).map_err(|_| WorkloadFailureKind::InvalidReport)
}

pub(super) fn verify_required_workload(
    report: &WorkloadReport,
    sql_log: &str,
    excluded_sql_fingerprints: &BTreeSet<String>,
) -> Result<WorkloadVerification, WorkloadFailureKind> {
    let required = unique_ids(&report.required_case_ids)?;
    let completed = unique_ids(&report.completed_case_ids)?;

    if required.is_empty() {
        return Err(WorkloadFailureKind::MissingCase);
    }

    let expectations = &report.expected_sql_by_case;
    if required
        .iter()
        .any(|case_id| !expectations.contains_key(case_id))
    {
        return Err(WorkloadFailureKind::MissingExpectation);
    }
    if expectations
        .keys()
        .any(|case_id| !required.contains(case_id))
    {
        return Err(WorkloadFailureKind::UnexpectedExpectation);
    }

    let mut expected_by_case = BTreeMap::<String, BTreeSet<String>>::new();
    for (case_id, fingerprints) in expectations {
        if fingerprints.is_empty() {
            return Err(WorkloadFailureKind::MissingExpectation);
        }
        let unique: BTreeSet<_> = fingerprints.iter().cloned().collect();
        if unique.len() != fingerprints.len() {
            return Err(WorkloadFailureKind::DuplicateExpectation);
        }
        if unique.iter().any(|fingerprint| !is_sha256(fingerprint)) {
            return Err(WorkloadFailureKind::InvalidReport);
        }
        expected_by_case.insert(case_id.clone(), unique);
    }

    let observed_by_case = parse_observed_sql(sql_log)?;
    if observed_by_case
        .keys()
        .any(|case_id| !required.contains(case_id))
    {
        return Err(WorkloadFailureKind::UnexpectedObservedCase);
    }

    let has_expected_observation = expected_by_case.iter().any(|(case_id, expected)| {
        observed_by_case.get(case_id).is_some_and(|observed| {
            expected
                .iter()
                .any(|fingerprint| observed.contains(fingerprint))
        })
    });
    let all_expected_excluded = expected_by_case.iter().all(|(case_id, expected)| {
        observed_by_case.get(case_id).is_none_or(BTreeSet::is_empty)
            && expected
                .iter()
                .all(|fingerprint| excluded_sql_fingerprints.contains(fingerprint))
    });
    if !has_expected_observation && all_expected_excluded {
        return Err(WorkloadFailureKind::ExcludedOnly);
    }

    if required.iter().any(|case_id| !completed.contains(case_id)) {
        return Err(WorkloadFailureKind::MissingCase);
    }
    if completed.iter().any(|case_id| !required.contains(case_id)) {
        return Err(WorkloadFailureKind::UnexpectedCompletedCase);
    }

    let mut observed_sql_count = 0;
    for (case_id, expected) in &expected_by_case {
        let Some(observed) = observed_by_case.get(case_id) else {
            return Err(WorkloadFailureKind::MissingObservation);
        };
        if expected
            .iter()
            .any(|fingerprint| !observed.contains(fingerprint))
        {
            return Err(WorkloadFailureKind::MissingObservation);
        }
        observed_sql_count += expected.len();
    }

    Ok(WorkloadVerification {
        case_count: required.len(),
        observed_sql_count,
    })
}

pub(super) fn sql_fingerprint(sql: &str) -> String {
    let normalized = normalize_sql(sql);
    let digest = Sha256::digest(normalized.as_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut fingerprint = String::with_capacity(64);
    for byte in digest {
        fingerprint.push(HEX[(byte >> 4) as usize] as char);
        fingerprint.push(HEX[(byte & 0x0f) as usize] as char);
    }
    fingerprint
}

fn unique_ids(ids: &[String]) -> Result<BTreeSet<String>, WorkloadFailureKind> {
    let unique: BTreeSet<_> = ids.iter().cloned().collect();
    if unique.len() != ids.len() {
        return Err(WorkloadFailureKind::DuplicateCaseId);
    }
    Ok(unique)
}

fn parse_observed_sql(
    contents: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, WorkloadFailureKind> {
    let mut observed = BTreeMap::<String, BTreeSet<String>>::new();

    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let entry: serde_json::Value =
            serde_json::from_str(line).map_err(|_| WorkloadFailureKind::InvalidSqlLog)?;
        let Some(application_name) = entry
            .get("application_name")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| value.strip_prefix(APPLICATION_NAME_PREFIX))
        else {
            continue;
        };
        let Some(sql) = sql_from_entry(&entry) else {
            continue;
        };
        if !is_select_or_with(&sql) {
            continue;
        }
        observed
            .entry(application_name.to_string())
            .or_default()
            .insert(sql_fingerprint(&sql));
    }

    Ok(observed)
}

fn sql_from_entry(entry: &serde_json::Value) -> Option<String> {
    let message = entry.get("message").and_then(serde_json::Value::as_str);
    let statement = entry.get("statement").and_then(serde_json::Value::as_str);
    let sql = if let Some(message) = message {
        if let Some(execute) = message.strip_prefix("execute ") {
            execute
                .split_once(": ")
                .map(|(_, sql)| sql.trim().to_string())
        } else if let Some(sql) = message.strip_prefix("statement: ") {
            Some(sql.trim().to_string())
        } else {
            statement.map(str::trim).map(str::to_string)
        }
    } else {
        statement.map(str::trim).map(str::to_string)
    };
    sql.filter(|value| !value.is_empty())
}

fn is_select_or_with(sql: &str) -> bool {
    let mut rest = sql.trim_start();
    loop {
        if let Some(comment) = rest.strip_prefix("--") {
            rest = comment
                .find('\n')
                .map(|newline| &comment[newline + 1..])
                .unwrap_or("")
                .trim_start();
            continue;
        }
        if let Some(comment) = rest.strip_prefix("/*") {
            let Some(end) = comment.find("*/") else {
                return false;
            };
            rest = comment[end + 2..].trim_start();
            continue;
        }

        let keyword: String = rest
            .chars()
            .take_while(|character| character.is_ascii_alphabetic())
            .collect();
        return keyword.eq_ignore_ascii_case("SELECT") || keyword.eq_ignore_ascii_case("WITH");
    }
}

fn normalize_sql(sql: &str) -> String {
    let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.trim_end_matches(';').trim_end().to_string()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use indoc::indoc;
    use rstest::rstest;

    use super::{
        WorkloadFailureKind, WorkloadReport, WorkloadVerification, parse_observed_sql,
        parse_workload_report, sql_fingerprint, verify_required_workload,
    };

    const SQL: &str = "SELECT * FROM sample_items WHERE id = $1";

    fn report(required: Vec<&str>, completed: Vec<&str>) -> WorkloadReport {
        let expected = sql_fingerprint(SQL);
        report_with_expected_sql(
            required,
            completed,
            BTreeMap::from([("case-a".to_string(), vec![expected])]),
        )
    }

    fn report_with_expected_sql(
        required: Vec<&str>,
        completed: Vec<&str>,
        expected_sql_by_case: BTreeMap<String, Vec<String>>,
    ) -> WorkloadReport {
        WorkloadReport {
            required_case_ids: required.into_iter().map(str::to_string).collect(),
            completed_case_ids: completed.into_iter().map(str::to_string).collect(),
            expected_sql_by_case,
        }
    }

    fn log(application_name: &str) -> String {
        format!(
            r#"{{"application_name":"{application_name}","message":"execute test: {SQL}","detail":"parameters: $1 = 'sample-value'"}}"#
        )
    }

    #[rstest]
    #[case::complete(report(vec!["case-a"], vec!["case-a"]), log("perf:case-a"), BTreeSet::new(), Ok(WorkloadVerification { case_count: 1, observed_sql_count: 1 }))]
    #[case::missing_completion(report(vec!["case-a"], vec![]), log("perf:case-a"), BTreeSet::new(), Err(WorkloadFailureKind::MissingCase))]
    #[case::missing_observation(report(vec!["case-a"], vec!["case-a"]), log("other:case-a"), BTreeSet::new(), Err(WorkloadFailureKind::MissingObservation))]
    #[case::missing_sql_log(report(vec!["case-a"], vec!["case-a"]), "", BTreeSet::new(), Err(WorkloadFailureKind::MissingObservation))]
    #[case::excluded_only(report(vec!["case-a"], vec!["case-a"]), "", BTreeSet::from([sql_fingerprint(SQL)]), Err(WorkloadFailureKind::ExcludedOnly))]
    fn verifies_required_cases_and_sql_observations(
        #[case] report: WorkloadReport,
        #[case] sql_log: String,
        #[case] excluded: BTreeSet<String>,
        #[case] expected: Result<WorkloadVerification, WorkloadFailureKind>,
    ) {
        assert_eq!(
            verify_required_workload(&report, &sql_log, &excluded),
            expected
        );
    }

    #[rstest]
    #[case::unexpected_completed(report(vec!["case-a"], vec!["case-a", "case-b"]), log("perf:case-a"), BTreeSet::new(), Err(WorkloadFailureKind::UnexpectedCompletedCase))]
    #[case::unexpected_expectation(report_with_expected_sql(vec!["case-a"], vec!["case-a"], BTreeMap::from([("case-a".to_string(), vec![sql_fingerprint(SQL)]), ("case-b".to_string(), vec![sql_fingerprint(SQL)])])), log("perf:case-a"), BTreeSet::new(), Err(WorkloadFailureKind::UnexpectedExpectation))]
    #[case::duplicate_expectation(report_with_expected_sql(vec!["case-a"], vec!["case-a"], BTreeMap::from([("case-a".to_string(), vec![sql_fingerprint(SQL), sql_fingerprint(SQL)])])), log("perf:case-a"), BTreeSet::new(), Err(WorkloadFailureKind::DuplicateExpectation))]
    #[case::invalid_fingerprint(report_with_expected_sql(vec!["case-a"], vec!["case-a"], BTreeMap::from([("case-a".to_string(), vec!["invalid".to_string()])])), log("perf:case-a"), BTreeSet::new(), Err(WorkloadFailureKind::InvalidReport))]
    #[case::unexpected_observed_case(report(vec!["case-a"], vec!["case-a"]), format!("{}\n{}", log("perf:case-a"), log("perf:case-b")), BTreeSet::new(), Err(WorkloadFailureKind::UnexpectedObservedCase))]
    #[case::invalid_sql_log(report(vec!["case-a"], vec!["case-a"]), "not-json".to_string(), BTreeSet::new(), Err(WorkloadFailureKind::InvalidSqlLog))]
    fn rejects_malformed_or_unexpected_workload_data(
        #[case] report: WorkloadReport,
        #[case] sql_log: String,
        #[case] excluded: BTreeSet<String>,
        #[case] expected: Result<WorkloadVerification, WorkloadFailureKind>,
    ) {
        assert_eq!(
            verify_required_workload(&report, &sql_log, &excluded),
            expected
        );
    }

    #[rstest]
    #[case::statement_message(format!(r#"{{"application_name":"perf:case-a","message":"statement: {SQL}"}}"#), Some(SQL))]
    #[case::statement_field(format!(r#"{{"application_name":"perf:case-a","statement":"{SQL}"}}"#), Some(SQL))]
    #[case::block_comment(format!(r#"{{"application_name":"perf:case-a","message":"execute prepared: /* sample comment */ {SQL}"}}"#), Some("/* sample comment */ SELECT * FROM sample_items WHERE id = $1"))]
    #[case::line_comment(r#"{"application_name":"perf:case-a","message":"execute prepared: -- sample comment\nWITH sample_rows AS (SELECT 1) SELECT * FROM sample_rows"}"#.to_string(), Some("-- sample comment\nWITH sample_rows AS (SELECT 1) SELECT * FROM sample_rows"))]
    #[case::non_select_is_ignored(r#"{"application_name":"perf:case-a","statement":"INSERT INTO sample_items (id) VALUES ($1)"}"#.to_string(), None)]
    fn parses_json_log_statement_forms(
        #[case] contents: String,
        #[case] expected_sql: Option<&str>,
    ) {
        let expected = expected_sql.map(|sql| {
            BTreeMap::from([("case-a".to_string(), BTreeSet::from([sql_fingerprint(sql)]))])
        });
        assert_eq!(
            parse_observed_sql(&contents),
            Ok(expected.unwrap_or_default())
        );
    }

    #[rstest]
    #[case::duplicate_required_id(indoc! {r#"{
        "required_case_ids": ["case-a", "case-a"],
        "completed_case_ids": ["case-a"],
        "expected_sql_by_case": {"case-a": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]}
    }"#}, Err(WorkloadFailureKind::DuplicateCaseId))]
    #[case::missing_expected_sql(indoc! {r#"{
        "required_case_ids": ["case-a"],
        "completed_case_ids": ["case-a"],
        "expected_sql_by_case": {}
    }"#}, Err(WorkloadFailureKind::MissingExpectation))]
    fn rejects_invalid_workload_reports(
        #[case] contents: &str,
        #[case] expected: Result<WorkloadReport, WorkloadFailureKind>,
    ) {
        let actual = parse_workload_report(contents).and_then(|report| {
            verify_required_workload(&report, "", &BTreeSet::new()).map(|_| report)
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn fingerprints_normalize_whitespace_before_hashing() {
        assert_eq!(
            sql_fingerprint(" SELECT\n * FROM sample_items WHERE id = $1; "),
            sql_fingerprint(SQL)
        );
    }
}
