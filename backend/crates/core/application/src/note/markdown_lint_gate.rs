use std::collections::HashSet;

use core_domain::markdown_lint::{Disposition, LintReport, Severity, evaluate_dispositions};
use core_domain::note_body_warning::{NoteBodyWarning, scan_graph_warnings_with_ids};

use crate::markdown_lint::AGENT_NOTE_WRITE;
use crate::note::types::{NoteBodyWarningDisposition, NoteWriteResult};
use crate::note::{NoteUseCaseError, NoteUseCases};

pub(super) struct CheckedLintOutcome {
    report: LintReport,
    warnings: Vec<NoteBodyWarning>,
}

impl NoteUseCases {
    pub(super) async fn inspect_checked_body(
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
        let report = self
            .markdown_lint
            .inspect(body_md, AGENT_NOTE_WRITE)
            .await?;
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
        let graph_warning_ids = graph_warnings
            .iter()
            .map(|warning| warning.id.clone())
            .collect::<Vec<_>>();
        let mut warnings = body_warnings;
        warnings.extend(graph_warnings);
        let dispositions = warning_dispositions
            .iter()
            .map(|disposition| Disposition {
                id: &disposition.id,
                reason: &disposition.reason,
            })
            .collect::<Vec<_>>();
        let evaluation = evaluate_dispositions(&report, &graph_warning_ids, &dispositions);
        let unresolved_warning_ids = evaluation
            .unresolved_warning_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let unresolved_warnings = warnings
            .iter()
            .filter(|warning| unresolved_warning_ids.contains(warning.id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let diagnostics = evaluation.unresolved_diagnostics;

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
