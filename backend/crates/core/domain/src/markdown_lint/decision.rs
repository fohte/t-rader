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
    pub accepted_dispositions: Vec<AcceptedDisposition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedDisposition {
    pub warning_id: String,
    pub reason: String,
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
    let mut disposition_reasons = HashMap::<String, Vec<(String, String)>>::new();
    for disposition in dispositions {
        if disposition.reason.trim().is_empty() {
            continue;
        }
        if let Some(group_id) = warning_id_group(disposition.id)
            && warning_counts.contains_key(group_id)
        {
            let group_id = group_id.to_string();
            if disposition_ids
                .entry(group_id.clone())
                .or_default()
                .insert(disposition.id.to_string())
            {
                disposition_reasons
                    .entry(group_id)
                    .or_default()
                    .push((disposition.id.to_string(), disposition.reason.to_string()));
            }
        }
    }

    let mut assigned_disposition_ids = HashMap::<String, HashSet<String>>::new();
    let mut accepted_dispositions = Vec::new();
    let mut unresolved_warning_ids = Vec::new();
    for id in &warning_ids {
        let Some(group_id) = warning_id_group(id) else {
            unresolved_warning_ids.push((*id).to_string());
            continue;
        };
        let Some(group_dispositions) = disposition_ids.get(group_id) else {
            unresolved_warning_ids.push((*id).to_string());
            continue;
        };
        let Some(group_reasons) = disposition_reasons.get(group_id) else {
            unresolved_warning_ids.push((*id).to_string());
            continue;
        };
        let disposition = group_reasons
            .iter()
            .find(|(disposition_id, _)| disposition_id == id)
            .or_else(|| {
                if group_dispositions.len() < warning_counts[group_id] {
                    return None;
                }
                let assigned = assigned_disposition_ids
                    .entry(group_id.to_string())
                    .or_default();
                group_reasons
                    .iter()
                    .find(|(disposition_id, _)| !assigned.contains(disposition_id))
                    .or_else(|| group_reasons.first())
            });
        let Some((disposition_id, reason)) = disposition else {
            unresolved_warning_ids.push((*id).to_string());
            continue;
        };
        assigned_disposition_ids
            .entry(group_id.to_string())
            .or_default()
            .insert(disposition_id.clone());
        accepted_dispositions.push(AcceptedDisposition {
            warning_id: (*id).to_string(),
            reason: reason.clone(),
        });
    }
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
        accepted_dispositions,
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rstest::rstest;

    use super::{AcceptedDisposition, Disposition, DispositionEvaluation, evaluate_dispositions};
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
                accepted_dispositions: if error {
                    Vec::new()
                } else {
                    vec![AcceptedDisposition {
                        warning_id: "price:sample-key:1".into(),
                        reason: "documented".into(),
                    }]
                },
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
                        reason: "first note",
                    },
                    Disposition {
                        id: "price:sample-key:5",
                        reason: "second note",
                    },
                    Disposition {
                        id: "price:sample-key:6",
                        reason: "third note",
                    },
                ],
            ),
            DispositionEvaluation {
                unresolved_diagnostics: Vec::new(),
                unresolved_warning_ids: Vec::new(),
                accepted_dispositions: vec![
                    AcceptedDisposition {
                        warning_id: "price:sample-key:1".into(),
                        reason: "first note".into(),
                    },
                    AcceptedDisposition {
                        warning_id: "price:sample-key:2".into(),
                        reason: "second note".into(),
                    },
                    AcceptedDisposition {
                        warning_id: "price:sample-key:3".into(),
                        reason: "third note".into(),
                    },
                ],
            },
        );
    }
}
