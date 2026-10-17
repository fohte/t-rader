//! ノート操作の inner method 実装。
//!

use core_application::change_history::{Actor, ChangeHistoryError};
use core_application::note::{
    NoteBodyWarningDisposition, NoteRepositoryError, NoteUseCaseError, NoteWriteCommand,
};
use core_application::note_kind::{NoteKindRepositoryError, NoteKindUseCaseError};
use core_application::unit_of_work::UnitOfWorkError;
use core_application::widget::WidgetDefinition;
use rmcp::ErrorData as McpError;

use super::dto::{
    ListNoteKindsResult, NoteKindDto, WriteNoteParams, WriteNoteResult, WriteNoteWarning,
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
                    "note body warnings must be fixed or dismissed with a reason:\n{warning_details}"
                ),
                Some(serde_json::json!({"warnings": warnings})),
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
        if let Some(body_md) = params.body_md.as_deref() {
            validate_widget_fenced_blocks(body_md)?;
        }

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
            .write_with_warning_dispositions(
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
        Ok(WriteNoteResult {
            note_id: result.note_id,
            created: result.created,
            warnings: core_domain::note_body_warning::scan_note_body_warnings_with_ids(
                &result.snapshot.version.body_md,
            )
            .into_iter()
            .map(|warning| WriteNoteWarning {
                id: warning.id,
                message: warning.message,
            })
            .collect(),
        })
    }
}

fn validate_widget_fenced_blocks(body_md: &str) -> Result<(), McpError> {
    let mut widget_block_count = 0;
    let mut active_fence = None;
    let mut widget_lines = Vec::new();

    for line in body_md.lines() {
        if let Some((marker, length, block_number)) = active_fence {
            if is_fence_close(line, marker, length) {
                if let Some(block_number) = block_number {
                    validate_widget_block(block_number, &widget_lines.join("\n"))?;
                    widget_lines.clear();
                }
                active_fence = None;
            } else if block_number.is_some() {
                widget_lines.push(line);
            }
            continue;
        }

        let Some((marker, length, info)) = parse_fence_marker(line) else {
            continue;
        };
        if marker == '`' && info.contains('`') {
            continue;
        }

        let block_number = if info == "widget" {
            widget_block_count += 1;
            Some(widget_block_count)
        } else {
            None
        };
        active_fence = Some((marker, length, block_number));
    }

    if let Some((_, _, Some(block_number))) = active_fence {
        validate_widget_block(block_number, &widget_lines.join("\n"))?;
    }

    Ok(())
}

fn parse_fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    if indentation > 3 {
        return None;
    }

    let candidate = &line[indentation..];
    let marker = candidate.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }

    let length = candidate.chars().take_while(|char| *char == marker).count();
    if length < 3 {
        return None;
    }

    Some((marker, length, candidate[length..].trim()))
}

fn is_fence_close(line: &str, marker: char, minimum_length: usize) -> bool {
    parse_fence_marker(line).is_some_and(|(found_marker, length, info)| {
        found_marker == marker && length >= minimum_length && info.is_empty()
    })
}

fn validate_widget_block(block_number: usize, yaml: &str) -> Result<(), McpError> {
    let definition = WidgetDefinition::from_yaml(yaml)
        .map_err(|error| invalid_params(format!("widget block {block_number}: {error}")))?;
    definition
        .validate()
        .map_err(|error| invalid_params(format!("widget block {block_number}: {error}")))?;
    if definition.time.as_of.is_none() {
        return Err(invalid_params(format!(
            "widget block {block_number}: time.as_of is required"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rmcp::model::ErrorCode;
    use serde_json::json;

    use super::note_use_case_to_mcp;
    use core_application::note::NoteUseCaseError;
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
                    "note body warnings must be fixed or dismissed with a reason:\n",
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
}
