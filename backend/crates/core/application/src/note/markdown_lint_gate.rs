use std::collections::{HashMap, HashSet};

use core_domain::markdown_lint::{Disposition, LintReport, Severity, evaluate_dispositions};
use core_domain::note_body_warning::{
    NoteBodyWarning, scan_graph_warnings_with_ids, warning_id_group,
};

use crate::markdown_lint::{AGENT_NOTE_WRITE, MarkdownLintService};
use crate::note::types::{NoteBodyWarningDisposition, NoteWriteResult};
use crate::note::{NoteUseCaseError, NoteUseCases};

pub(super) struct CheckedLintOutcome {
    report: LintReport,
    warnings: Vec<NoteBodyWarning>,
}

impl NoteUseCases {
    pub(super) fn inspect_checked_body(
        &self,
        body_md: &str,
        graphs_json: &serde_json::Value,
        warning_dispositions: Option<&[NoteBodyWarningDisposition]>,
    ) -> Result<Option<CheckedLintOutcome>, NoteUseCaseError> {
        let Some(warning_dispositions) = warning_dispositions else {
            return Ok(None);
        };
        let graphs: Vec<core_domain::note_graph::GraphDef> =
            serde_json::from_value(graphs_json.clone()).map_err(|error| {
                NoteUseCaseError::Validation(format!("invalid graphs_json: {error}"))
            })?;
        let report = MarkdownLintService.inspect(body_md, AGENT_NOTE_WRITE)?;
        let body_warnings = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .map(|diagnostic| NoteBodyWarning {
                id: diagnostic.id.clone(),
                message: diagnostic.message.clone(),
            })
            .collect::<Vec<_>>();
        let graph_warnings = scan_graph_warnings_with_ids(&graphs, &body_warnings);
        let mut warnings = body_warnings;
        warnings.extend(graph_warnings);
        let unresolved_warnings = unresolved_warning_dispositions(&warnings, warning_dispositions);
        let dispositions = warning_dispositions
            .iter()
            .map(|disposition| Disposition {
                id: &disposition.id,
                reason: &disposition.reason,
            })
            .collect::<Vec<_>>();
        let evaluator_diagnostics = match evaluate_dispositions(&report, &dispositions) {
            Ok(()) => Vec::new(),
            Err(diagnostics) => diagnostics,
        };
        let unresolved_error_ids = evaluator_diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| diagnostic.id)
            .collect::<HashSet<_>>();
        let unresolved_warning_ids = unresolved_warnings
            .iter()
            .map(|warning| warning.id.as_str())
            .collect::<HashSet<_>>();
        let diagnostics = report
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                unresolved_error_ids.contains(&diagnostic.id)
                    || unresolved_warning_ids.contains(diagnostic.id.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();

        if !diagnostics.is_empty() || !unresolved_warnings.is_empty() {
            return Err(NoteUseCaseError::BodyLintRejected {
                diagnostics,
                warnings: unresolved_warnings,
            });
        }

        Ok(Some(CheckedLintOutcome { report, warnings }))
    }
}

pub(super) fn apply_lint_outcome(
    result: &mut NoteWriteResult,
    lint_outcome: Option<CheckedLintOutcome>,
) {
    if let Some(lint_outcome) = lint_outcome {
        result.lint_report = Some(lint_outcome.report);
        result.warnings = lint_outcome.warnings;
    }
}

fn unresolved_warning_dispositions(
    warnings: &[NoteBodyWarning],
    warning_dispositions: &[NoteBodyWarningDisposition],
) -> Vec<NoteBodyWarning> {
    let warning_counts = warnings.iter().fold(HashMap::new(), |mut counts, warning| {
        if let Some(group_id) = warning_id_group(&warning.id) {
            *counts.entry(group_id.to_string()).or_insert(0_usize) += 1;
        }
        counts
    });
    let disposition_ids = warning_dispositions
        .iter()
        .filter(|disposition| !disposition.reason.trim().is_empty())
        .filter_map(|disposition| {
            warning_id_group(&disposition.id)
                .filter(|group_id| warning_counts.contains_key(*group_id))
                .map(|group_id| (group_id.to_string(), disposition.id.clone()))
        })
        .fold(
            HashMap::<String, HashSet<String>>::new(),
            |mut groups, (group_id, id)| {
                groups.entry(group_id).or_default().insert(id);
                groups
            },
        );
    warnings
        .iter()
        .filter(|warning| {
            let Some(group_id) = warning_id_group(&warning.id) else {
                return true;
            };
            let Some(group_dispositions) = disposition_ids.get(group_id) else {
                return true;
            };
            if group_dispositions.contains(&warning.id) {
                return false;
            }
            group_dispositions.len() < warning_counts[group_id]
        })
        .cloned()
        .collect()
}
