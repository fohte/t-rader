//! ノート操作の inner method 実装。
//!
//! 戦略境界の検査は [`super::fetch_note_owned_by`] が担う。

use core_application::change_history::Actor;
use core_application::note::{NoteUseCaseError, NoteWriteCommand};
use core_application::strategy_scope::StrategyScope;
use rmcp::ErrorData as McpError;

use super::dto::{ListNoteKindsResult, NoteKindDto, WriteNoteParams, WriteNoteResult};
use super::{
    STRATEGY_AGENT_ACTOR, StrategyServer, app_error_to_mcp, internal_error, invalid_params,
};

mod read;

fn note_use_case_to_mcp(error: NoteUseCaseError) -> McpError {
    match error {
        NoteUseCaseError::Validation(message) => invalid_params(message),
        NoteUseCaseError::UnknownNoteKind(kind) => {
            invalid_params(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::ReferencedNoteKindNotFound(kind) => {
            internal_error(format!("note kind {kind} not found"))
        }
        NoteUseCaseError::NotFound(_) => McpError::resource_not_found("note not found", None),
        NoteUseCaseError::Forbidden(note_id) => invalid_params(format!(
            "forbidden: note {note_id} belongs to another strategy"
        )),
        other => internal_error(format!("{other}")),
    }
}

impl StrategyServer {
    pub(crate) async fn list_note_kinds_inner(&self) -> Result<ListNoteKindsResult, McpError> {
        let note_kinds = self
            .use_cases
            .note_kinds()
            .list()
            .await
            .map_err(|error| app_error_to_mcp(error.into()))?
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
        scope: impl Into<StrategyScope>,
        execution_id: Option<String>,
        params: WriteNoteParams,
    ) -> Result<WriteNoteResult, McpError> {
        let scope = scope.into();
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
            .use_cases
            .notes()
            .write(NoteWriteCommand {
                scope: Some(scope),
                strategy_id: Some(scope.id()),
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
            })
            .await
            .map_err(note_use_case_to_mcp)?;
        Ok(WriteNoteResult {
            note_id: result.note_id,
            created: result.created,
        })
    }
}

#[cfg(test)]
include!("notes/tests.rs");
