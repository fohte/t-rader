use std::collections::{HashMap, HashSet};

use super::id::warning_id_group;
use super::types::{LintDiagnostic, LintReport, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposition<'a> {
    pub id: &'a str,
    pub reason: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispositionEvaluation {
    pub unresolved_diagnostics: Vec<LintDiagnostic>,
    pub unresolved_warning_ids: Vec<String>,
}

pub fn evaluate_dispositions(
    report: &LintReport,
    additional_warning_ids: &[String],
    dispositions: &[Disposition<'_>],
) -> DispositionEvaluation {
    let warning_ids = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .map(|diagnostic| diagnostic.id.as_str())
        .chain(additional_warning_ids.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let mut warning_counts = HashMap::<String, usize>::new();
    for id in &warning_ids {
        if let Some(group_id) = warning_id_group(id) {
            *warning_counts.entry(group_id.to_string()).or_default() += 1;
        }
    }

    let mut disposition_ids = HashMap::<String, HashSet<String>>::new();
    for disposition in dispositions {
        if disposition.reason.trim().is_empty() {
            continue;
        }
        if let Some(group_id) = warning_id_group(disposition.id)
            && warning_counts.contains_key(group_id)
        {
            disposition_ids
                .entry(group_id.to_string())
                .or_default()
                .insert(disposition.id.to_string());
        }
    }

    let is_disposed = |id: &str| {
        let Some(group_id) = warning_id_group(id) else {
            return false;
        };
        let Some(group_dispositions) = disposition_ids.get(group_id) else {
            return false;
        };
        group_dispositions.contains(id) || group_dispositions.len() >= warning_counts[group_id]
    };
    let unresolved_warning_ids = warning_ids
        .iter()
        .filter(|id| !is_disposed(id))
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    let unresolved_warning_id_set = unresolved_warning_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let unresolved_diagnostics = report
        .diagnostics
        .iter()
        .filter(|diagnostic| match diagnostic.severity {
            Severity::Error => true,
            Severity::Warning => unresolved_warning_id_set.contains(diagnostic.id.as_str()),
        })
        .cloned()
        .collect::<Vec<_>>();

    DispositionEvaluation {
        unresolved_diagnostics,
        unresolved_warning_ids,
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rstest::rstest;

    use super::{Disposition, DispositionEvaluation, evaluate_dispositions};
    use crate::markdown_lint::{LintDiagnostic, LintReport, Severity};

    fn diagnostic(rule_id: &'static str, id: &str, severity: Severity) -> LintDiagnostic {
        LintDiagnostic {
            rule_id,
            id: id.to_string(),
            severity,
            range: Range { start: 0, end: 1 },
            message: "sample finding".into(),
            fix: None,
        }
    }

    #[rstest]
    #[case::error_ignores_reason(true, Some(Disposition { id: "sample-rule:sample-key:1", reason: "documented" }))]
    #[case::error_without_reason(true, None)]
    #[case::warning_with_reason(false, Some(Disposition { id: "price:sample-key:1", reason: "documented" }))]
    fn evaluates_diagnostics(
        #[case] error: bool,
        #[case] disposition: Option<Disposition<'static>>,
    ) {
        let severity = if error {
            Severity::Error
        } else {
            Severity::Warning
        };
        let report = LintReport {
            diagnostics: vec![diagnostic(
                if error { "sample-rule" } else { "price" },
                if error {
                    "sample-rule:sample-key:1"
                } else {
                    "price:sample-key:1"
                },
                severity,
            )],
        };
        let dispositions = disposition.into_iter().collect::<Vec<_>>();

        assert_eq!(
            evaluate_dispositions(&report, &[], &dispositions),
            DispositionEvaluation {
                unresolved_diagnostics: if error {
                    vec![diagnostic(
                        "sample-rule",
                        "sample-rule:sample-key:1",
                        Severity::Error,
                    )]
                } else {
                    Vec::new()
                },
                unresolved_warning_ids: Vec::new(),
            },
        );
    }

    #[test]
    fn preserves_occurrence_count_matching_for_warning_dispositions() {
        let report = LintReport {
            diagnostics: vec![
                diagnostic("price", "price:sample-key:1", Severity::Warning),
                diagnostic("price", "price:sample-key:2", Severity::Warning),
            ],
        };
        assert_eq!(
            evaluate_dispositions(
                &report,
                &["price:sample-key:3".into()],
                &[
                    Disposition {
                        id: "price:sample-key:4",
                        reason: "documented",
                    },
                    Disposition {
                        id: "price:sample-key:5",
                        reason: "documented",
                    },
                    Disposition {
                        id: "price:sample-key:6",
                        reason: "documented",
                    },
                ],
            ),
            DispositionEvaluation {
                unresolved_diagnostics: Vec::new(),
                unresolved_warning_ids: Vec::new(),
            },
        );
    }
}
