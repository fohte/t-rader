//! ノート操作の inner method 実装。
//!

use core_application::change_history::{Actor, ChangeHistoryError};
use core_application::note::{
    NoteBodyWarningDisposition, NoteRepositoryError, NoteUseCaseError, NoteWriteCommand,
};
use core_application::note_kind::{NoteKindRepositoryError, NoteKindUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use core_domain::markdown_lint::{LintDiagnostic, Severity};
use rmcp::ErrorData as McpError;

use super::dto::{
    ListNoteKindsResult, NoteKindDto, WriteNoteDiagnostic, WriteNoteDiagnosticSeverity,
    WriteNoteParams, WriteNoteRange, WriteNoteResult, WriteNoteTextEdit, WriteNoteWarning,
};
use super::{
    STRATEGY_AGENT_ACTOR, StrategyServer, internal_error, internal_failure, invalid_params,
};

mod read;

pub(crate) use read::note_read_error_to_mcp;

fn note_use_case_to_mcp(error: NoteUseCaseError) -> McpError {
    match error {
        NoteUseCaseError::Validation(message) => invalid_params(message),
        NoteUseCaseError::UnresolvedBodyWarnings { warnings } => {
            let warning_details = warnings
                .iter()
                .map(|warning| format!("{}: {}", warning.id, warning.message))
                .collect::<Vec<_>>()
                .join("\n");
            McpError::invalid_params(
                format!(
                    "note warnings must be fixed or dismissed with a reason:\n{warning_details}"
                ),
                Some(serde_json::json!({"warnings": warnings})),
            )
        }
        NoteUseCaseError::BodyLintRejected {
            diagnostics,
            warnings,
        } => {
            let has_error = diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error);
            let details = diagnostics
                .iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.id, diagnostic.message))
                .collect::<Vec<_>>()
                .join("\n");
            let message = if has_error {
                format!("note body contains blocking lint diagnostics:\n{details}")
            } else {
                let warning_details = warnings
                    .iter()
                    .map(|warning| format!("{}: {}", warning.id, warning.message))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!(
                    "note warnings must be fixed or dismissed with a reason:\n{warning_details}"
                )
            };
            McpError::invalid_params(
                message,
                Some(serde_json::json!({"warnings": warnings, "diagnostics": diagnostics})),
            )
        }
        NoteUseCaseError::UnknownNoteKind(kind) => {
            invalid_params(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::ReferencedNoteKindNotFound(kind) => {
            internal_error(format!("note kind {kind} not found"))
        }
        NoteUseCaseError::NotFound(_) => McpError::resource_not_found("note not found", None),
        other => internal_error(format!("{other}")),
    }
}

fn note_kind_use_case_to_mcp(error: NoteKindUseCaseError) -> McpError {
    match error {
        NoteKindUseCaseError::Validation(message) => invalid_params(message),
        NoteKindUseCaseError::NotFound(message) => internal_error(format!("not found: {message}")),
        NoteKindUseCaseError::Conflict(message) => internal_error(format!("conflict: {message}")),
        NoteKindUseCaseError::Repository(NoteKindRepositoryError::Database(error))
        | NoteKindUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | NoteKindUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | NoteKindUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
            super::persistence_error_to_mcp(error)
        }
        NoteKindUseCaseError::Note(error) => note_kind_note_use_case_to_mcp(error),
        other => internal_failure(&other.to_string()),
    }
}

fn note_kind_note_use_case_to_mcp(error: NoteUseCaseError) -> McpError {
    match error {
        NoteUseCaseError::Validation(message) => invalid_params(message),
        NoteUseCaseError::UnknownNoteKind(kind) => {
            invalid_params(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::NotFound(message) => internal_error(format!("not found: {message}")),
        NoteUseCaseError::ReferencedNoteKindNotFound(kind) => {
            internal_error(format!("not found: note kind {kind} not found"))
        }
        NoteUseCaseError::Conflict(message) => internal_error(format!("conflict: {message}")),
        NoteUseCaseError::Repository(NoteRepositoryError::Database(error))
        | NoteUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Begin(error))
        | NoteUseCaseError::UnitOfWork(UnitOfWorkError::Commit(error)) => {
            super::persistence_error_to_mcp(error)
        }
        other => internal_failure(&other.to_string()),
    }
}

impl StrategyServer {
    pub(crate) async fn list_note_kinds_inner(&self) -> Result<ListNoteKindsResult, McpError> {
        let note_kinds = self
            .dependencies
            .note_kinds
            .list()
            .await
            .map_err(note_kind_use_case_to_mcp)?
            .into_iter()
            .map(|kind| NoteKindDto {
                key: kind.key,
                display_name: kind.display_name,
                requires_approval: kind.requires_approval,
                description: kind.description,
                sort_order: kind.sort_order,
            })
            .collect();
        Ok(ListNoteKindsResult { note_kinds })
    }

    pub(crate) async fn write_note_inner(
        &self,
        execution_id: Option<String>,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        let warning_dispositions = params
            .warning_dispositions
            .into_iter()
            .map(|disposition| NoteBodyWarningDisposition {
                id: disposition.id,
                reason: disposition.reason,
            })
            .collect();
        let graphs_json = params
            .graphs
            .map(|graphs| {
                let graphs: Vec<core_domain::note_graph::GraphDef> =
                    graphs.into_iter().map(Into::into).collect();
                serde_json::to_value(graphs)
                    .map_err(|error| internal_error(format!("failed to serialize graphs: {error}")))
            })
            .transpose()?;
        let result = self
            .dependencies
            .notes
            .write_agent_note_with_warning_dispositions(
                NoteWriteCommand {
                    execution_id,
                    note_id: params.note_id,
                    title: params.title,
                    body_md: params.body_md,
                    frontmatter_json: params.frontmatter_json.map(serde_json::Value::Object),
                    graphs_json,
                    kind: params.kind,
                    status: None,
                    trigger: None,
                    trigger_label: None,
                    created_by_kind: STRATEGY_AGENT_ACTOR.into(),
                    change_reason: params.change_reason,
                    actor: Actor::Llm {
                        label: STRATEGY_AGENT_ACTOR,
                    },
                    change_diff: None,
                },
                warning_dispositions,
            )
            .await
            .map_err(note_use_case_to_mcp)?;
        let warnings = result
            .warnings
            .into_iter()
            .map(|warning| WriteNoteWarning {
                id: warning.id,
                message: warning.message,
            })
            .collect();
        let diagnostics = result
            .lint_report
            .map_or_else(Vec::new, |report| report.diagnostics)
            .into_iter()
            .map(write_note_diagnostic)
            .collect();
        Ok(WriteNoteResult {
            note_id: result.note_id,
            created: result.created,
            warnings,
            diagnostics,
        })
    }
}

fn write_note_diagnostic(diagnostic: LintDiagnostic) -> WriteNoteDiagnostic {
    WriteNoteDiagnostic {
        rule_id: diagnostic.rule_id.into(),
        id: diagnostic.id,
        severity: match diagnostic.severity {
            Severity::Warning => WriteNoteDiagnosticSeverity::Warning,
            Severity::Error => WriteNoteDiagnosticSeverity::Error,
        },
        range: WriteNoteRange {
            start: diagnostic.range.start,
            end: diagnostic.range.end,
        },
        message: diagnostic.message,
        fix: diagnostic.fix.map(|fix| WriteNoteTextEdit {
            range: WriteNoteRange {
                start: fix.range.start,
                end: fix.range.end,
            },
            replacement: fix.replacement,
        }),
    }
}

#[cfg(test)]
mod tests {
    use rmcp::model::ErrorCode;
    use serde_json::json;

    use super::note_use_case_to_mcp;
    use core_application::note::NoteUseCaseError;
    use core_domain::markdown_lint::{LintDiagnostic, Severity};
    use core_domain::note_body_warning::NoteBodyWarning;

    #[test]
    fn unresolved_warning_errors_include_warning_ids_in_data() {
        let error = note_use_case_to_mcp(NoteUseCaseError::UnresolvedBodyWarnings {
            warnings: vec![NoteBodyWarning {
                id: "price:45%2C678%E5%86%86:1".into(),
                message: "価格候補の数値「45,678円」がリンク外にあります。".into(),
            }],
        });

        assert_eq!(
            (error.code, error.message.to_string(), error.data),
            (
                ErrorCode::INVALID_PARAMS,
                concat!(
                    "note warnings must be fixed or dismissed with a reason:\n",
                    "price:45%2C678%E5%86%86:1: 価格候補の数値「45,678円」がリンク外にあります。"
                )
                .into(),
                Some(json!({
                    "warnings": [{
                        "id": "price:45%2C678%E5%86%86:1",
                        "message": "価格候補の数値「45,678円」がリンク外にあります。",
                    }],
                })),
            ),
        );
    }

    #[test]
    fn lint_rejections_return_error_diagnostics_and_warning_subset() {
        let warning = LintDiagnostic {
            rule_id: "price",
            id: "price:12%2C345%E5%86%86:1".into(),
            severity: Severity::Warning,
            range: 0..9,
            message: "sample price warning".into(),
            fix: None,
        };
        let error_diagnostic = LintDiagnostic {
            rule_id: "sample-rule",
            id: "sample-rule:sample-key:1".into(),
            severity: Severity::Error,
            range: 10..15,
            message: "sample blocking diagnostic".into(),
            fix: None,
        };
        let error = note_use_case_to_mcp(NoteUseCaseError::BodyLintRejected {
            diagnostics: vec![warning.clone(), error_diagnostic],
            warnings: vec![NoteBodyWarning {
                id: warning.id.clone(),
                message: warning.message.clone(),
            }],
        });

        assert_eq!(
            (error.code, error.message.to_string(), error.data),
            (
                ErrorCode::INVALID_PARAMS,
                concat!(
                    "note body contains blocking lint diagnostics:\n",
                    "price:12%2C345%E5%86%86:1: sample price warning\n",
                    "sample-rule:sample-key:1: sample blocking diagnostic"
                )
                .into(),
                Some(json!({
                    "warnings": [{
                        "id": "price:12%2C345%E5%86%86:1",
                        "message": "sample price warning",
                    }],
                    "diagnostics": [
                        {
                            "rule_id": "price",
                            "id": "price:12%2C345%E5%86%86:1",
                            "severity": "warning",
                            "range": {"start": 0, "end": 9},
                            "message": "sample price warning",
                            "fix": null,
                        },
                        {
                            "rule_id": "sample-rule",
                            "id": "sample-rule:sample-key:1",
                            "severity": "error",
                            "range": {"start": 10, "end": 15},
                            "message": "sample blocking diagnostic",
                            "fix": null,
                        },
                    ],
                })),
            ),
        );
    }
}
