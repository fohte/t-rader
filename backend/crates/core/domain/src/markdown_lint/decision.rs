use std::collections::{HashMap, HashSet};

use super::id::warning_id_group;
use super::types::{LintDiagnostic, LintReport, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposition<'a> {
    pub id: &'a str,
    pub reason: &'a str,
}

pub fn evaluate_dispositions(
    report: &LintReport,
    dispositions: &[Disposition<'_>],
) -> Result<(), Vec<LintDiagnostic>> {
    let mut warning_counts = HashMap::<String, usize>::new();
    for diagnostic in &report.diagnostics {
        if diagnostic.severity == Severity::Warning
            && let Some(group_id) = warning_id_group(&diagnostic.id)
        {
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

    let unresolved = report
        .diagnostics
        .iter()
        .filter(|diagnostic| match diagnostic.severity {
            Severity::Error => true,
            Severity::Warning => {
                let Some(group_id) = warning_id_group(&diagnostic.id) else {
                    return true;
                };
                let Some(group_dispositions) = disposition_ids.get(group_id) else {
                    return true;
                };
                !group_dispositions.contains(&diagnostic.id)
                    && group_dispositions.len() < warning_counts[group_id]
            }
        })
        .cloned()
        .collect::<Vec<_>>();

    if unresolved.is_empty() {
        Ok(())
    } else {
        Err(unresolved)
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use rstest::rstest;

    use super::{Disposition, evaluate_dispositions};
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
            evaluate_dispositions(&report, &dispositions).map_err(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.id)
                    .collect::<Vec<_>>()
            }),
            if error {
                Err(vec!["sample-rule:sample-key:1".to_string()])
            } else {
                Ok(())
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
                &[
                    Disposition {
                        id: "price:sample-key:3",
                        reason: "documented",
                    },
                    Disposition {
                        id: "price:sample-key:4",
                        reason: "documented",
                    },
                ],
            ),
            Ok(()),
        );
    }
}
