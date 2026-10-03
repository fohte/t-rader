//! 管理 MCP のノート種別一覧 tool。

use rmcp::ErrorData as McpError;

use core_application::change_history::ChangeHistoryError;
use core_application::note::{NoteRepositoryError, NoteUseCaseError};
use core_application::note_kind::{NoteKindRepositoryError, NoteKindUseCaseError};
use core_application::persistence::PersistenceError;
use core_application::unit_of_work::UnitOfWorkError;

use super::MgmtServer;
use super::dto::ListNoteKindsResult;
use super::{internal_failure, invalid_params};

impl MgmtServer {
    pub(super) async fn list_note_kinds_inner(&self) -> Result<ListNoteKindsResult, McpError> {
        let rows = self
            .dependencies
            .note_kinds
            .list()
            .await
            .map_err(map_note_kind_error)?;
        Ok(ListNoteKindsResult {
            note_kinds: rows.into_iter().map(Into::into).collect(),
        })
    }
}

fn map_note_kind_error(err: NoteKindUseCaseError) -> McpError {
    match err {
        NoteKindUseCaseError::Validation(message) | NoteKindUseCaseError::Conflict(message) => {
            invalid_params(message)
        }
        NoteKindUseCaseError::NotFound(message) => McpError::resource_not_found(message, None),
        NoteKindUseCaseError::Repository(NoteKindRepositoryError::Database(error))
        | NoteKindUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | NoteKindUseCaseError::UnitOfWork(
            UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error),
        ) => map_note_kind_persistence_error(error),
        NoteKindUseCaseError::Note(error) => map_note_use_case_error(error),
        other => internal_failure(&other.to_string()),
    }
}

fn map_note_use_case_error(error: NoteUseCaseError) -> McpError {
    match error {
        NoteUseCaseError::Validation(message) => invalid_params(message),
        NoteUseCaseError::UnknownNoteKind(kind) => {
            invalid_params(format!("unknown note kind: {kind}"))
        }
        NoteUseCaseError::NotFound(message) => McpError::resource_not_found(message, None),
        NoteUseCaseError::ReferencedNoteKindNotFound(key) => {
            McpError::resource_not_found(format!("note kind {key} not found"), None)
        }
        NoteUseCaseError::Conflict(message) => invalid_params(message),
        NoteUseCaseError::Repository(NoteRepositoryError::Database(error))
        | NoteUseCaseError::ChangeHistory(ChangeHistoryError::Database(error))
        | NoteUseCaseError::UnitOfWork(
            UnitOfWorkError::Begin(error) | UnitOfWorkError::Commit(error),
        ) => map_note_kind_persistence_error(error),
        other => internal_failure(&other.to_string()),
    }
}

fn map_note_kind_persistence_error(error: PersistenceError) -> McpError {
    match error {
        PersistenceError::Database(message) => internal_failure(&message),
        PersistenceError::MissingReference(_) => {
            invalid_params("referenced resource does not exist")
        }
        PersistenceError::Conflict(_) => invalid_params("resource already exists"),
        PersistenceError::ConstraintViolation(_) => {
            invalid_params("value violates database constraint")
        }
        PersistenceError::RecordNotUpdated(_) => {
            McpError::resource_not_found("resource not found", None)
        }
    }
}
