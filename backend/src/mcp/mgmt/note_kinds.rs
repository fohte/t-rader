//! 管理 MCP のノート種別一覧 tool。

use rmcp::ErrorData as McpError;

use crate::error::AppError;
use core_application::note_kind::NoteKindUseCaseError;

use super::MgmtServer;
use super::dto::ListNoteKindsResult;
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
}

fn map_note_kind_error(err: NoteKindUseCaseError) -> McpError {
    match AppError::from(err) {
        AppError::Validation(message) | AppError::Conflict(message) => invalid_params(message),
        AppError::NotFound(message) => McpError::resource_not_found(message, None),
        AppError::Database(database_error) => super::db_error(database_error),
        other => super::internal_error(other.to_string()),
    }
}
