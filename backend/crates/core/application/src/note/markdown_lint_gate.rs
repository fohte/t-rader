use std::collections::HashSet;

use core_domain::markdown_lint::{Disposition, LintReport, Severity, evaluate_dispositions};
use core_domain::note_body_warning::{NoteBodyWarning, scan_graph_warning_findings_with_ids};

use crate::markdown_lint::AGENT_NOTE_WRITE;
use crate::note::types::{
    NoteBodyWarningDisposition, NoteWarningDispositionLocation, NoteWarningDispositionRecord,
    NoteWriteResult,
};
use crate::note::{NoteUseCaseError, NoteUseCases};

pub(super) struct CheckedLintOutcome {
    report: LintReport,
    warnings: Vec<NoteBodyWarning>,
    pub(super) accepted_dispositions: Vec<NoteWarningDispositionRecord>,
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
        let graph_warning_findings = scan_graph_warning_findings_with_ids(&graphs, &body_warnings);
        let graph_warnings = graph_warning_findings
            .iter()
            .map(|finding| finding.warning.clone())
            .collect::<Vec<_>>();
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

        let accepted_reasons = evaluation
            .accepted_dispositions
            .into_iter()
            .map(|disposition| (disposition.warning_id, disposition.reason))
            .collect::<std::collections::HashMap<_, _>>();
        let mut accepted_dispositions = Vec::new();
        for diagnostic in report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        {
            let Some(reason) = accepted_reasons.get(&diagnostic.id) else {
                continue;
            };
            let value = body_md
                .get(diagnostic.range.clone())
                .ok_or_else(|| {
                    NoteUseCaseError::Validation(format!(
                        "warning range is outside note body: {}..{}",
                        diagnostic.range.start, diagnostic.range.end
                    ))
                })?
                .to_string();
            accepted_dispositions.push(NoteWarningDispositionRecord {
                warning_id: diagnostic.id.clone(),
                value,
                location: NoteWarningDispositionLocation::Body {
                    start: diagnostic.range.start,
                    end: diagnostic.range.end,
                },
                reason: reason.clone(),
            });
        }
        for finding in graph_warning_findings {
            let Some(reason) = accepted_reasons.get(&finding.warning.id) else {
                continue;
            };
            let path = finding.path.ok_or_else(|| {
                NoteUseCaseError::Validation(format!(
                    "graph warning has no source path: {}",
                    finding.warning.id
                ))
            })?;
            accepted_dispositions.push(NoteWarningDispositionRecord {
                warning_id: finding.warning.id,
                value: finding.value,
                location: NoteWarningDispositionLocation::Graph {
                    path,
                    start: finding.range.start,
                    end: finding.range.end,
                },
                reason: reason.clone(),
            });
        }

        Ok(Some(CheckedLintOutcome {
            report,
            warnings,
            accepted_dispositions,
        }))
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
