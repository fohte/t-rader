//! 管理 MCP のノート種別 CRUD tool。

use rmcp::ErrorData as McpError;

use crate::error::AppError;
use core_application::change_history::Actor;
use core_application::note_kind::{
    CreateNoteKindCommand, NoteKindUseCaseError, UpdateNoteKindCommand,
};

use super::MgmtServer;
use super::dto::{
    CreateNoteKindParams, DeleteNoteKindParams, DeleteNoteKindResult, ListNoteKindsResult,
    NoteKindSummary, UpdateNoteKindParams,
};
use super::invalid_params;

impl MgmtServer {
    pub(super) async fn list_note_kinds_inner(&self) -> Result<ListNoteKindsResult, McpError> {
        let rows = self
            .use_cases
            .note_kinds()
            .list()
            .await
            .map_err(map_note_kind_error)?;
        Ok(ListNoteKindsResult {
            note_kinds: rows.into_iter().map(Into::into).collect(),
        })
    }

    pub(super) async fn create_note_kind_inner(
        &self,
        params: CreateNoteKindParams,
    ) -> Result<NoteKindSummary, McpError> {
        let created = self
            .use_cases
            .note_kinds()
            .create(
                Actor::Llm { label: "mgmt-mcp" },
                CreateNoteKindCommand {
                    key: params.key,
                    display_name: params.display_name,
                    requires_approval: params.requires_approval.unwrap_or(false),
                    description: params.description,
                    sort_order: params.sort_order,
                },
            )
            .await
            .map_err(map_note_kind_error)?;
        Ok(created.into())
    }

    pub(super) async fn update_note_kind_inner(
        &self,
        params: UpdateNoteKindParams,
    ) -> Result<NoteKindSummary, McpError> {
        let updated = self
            .use_cases
            .note_kinds()
            .update(
                Actor::Llm { label: "mgmt-mcp" },
                &params.key,
                UpdateNoteKindCommand {
                    display_name: params.display_name,
                    requires_approval: params.requires_approval,
                    description: params.description,
                    sort_order: params.sort_order,
                },
            )
            .await
            .map_err(map_note_kind_error)?;
        Ok(updated.into())
    }

    pub(super) async fn delete_note_kind_inner(
        &self,
        params: DeleteNoteKindParams,
    ) -> Result<DeleteNoteKindResult, McpError> {
        self.use_cases
            .note_kinds()
            .delete(Actor::Llm { label: "mgmt-mcp" }, &params.key)
            .await
            .map_err(map_note_kind_error)?;
        Ok(DeleteNoteKindResult { key: params.key })
    }
}

fn map_note_kind_error(err: NoteKindUseCaseError) -> McpError {
    match AppError::from(err) {
        AppError::Validation(message) | AppError::Conflict(message) => invalid_params(message),
        AppError::NotFound(message) => McpError::resource_not_found(message, None),
        AppError::Database(database_error) => super::db_error(database_error),
        other => super::internal_error(other.to_string()),
    }
}
