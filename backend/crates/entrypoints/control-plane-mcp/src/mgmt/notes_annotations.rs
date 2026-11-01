//! 管理 MCP の直近ノート・アノテーション一覧 tool。

use rmcp::ErrorData as McpError;

use core_application::annotation::{AnnotationReadQueryError, AnnotationReadUseCaseError};
use core_application::note::{NoteOverviewQuery, NoteReadQueryError, NoteReadUseCaseError};

use super::MgmtServer;
use super::dto::{
    AnnotationMeta, ListRecentAnnotationsResult, ListRecentNotesResult, ListRecentParams, NoteMeta,
};
use super::{clamp_limit, internal_error, invalid_params, map_persistence_error};

fn note_read_error_to_mcp(error: NoteReadUseCaseError) -> McpError {
    match error {
        NoteReadUseCaseError::NotFound(_) => McpError::resource_not_found("note not found", None),
        NoteReadUseCaseError::VersionDoesNotBelong {
            note_id,
            version_id,
        } => invalid_params(format!(
            "version_id {version_id} does not belong to note {note_id}"
        )),
        NoteReadUseCaseError::NoVersion(note_id) => {
            internal_error(format!("note {note_id} has no version"))
        }
        NoteReadUseCaseError::InitialVersionNotFound(note_id) => {
            internal_error(format!("note {note_id} has no initial version"))
        }
        NoteReadUseCaseError::VersionNumberNotFound { .. }
        | NoteReadUseCaseError::NoteVersionNotFound => {
            McpError::resource_not_found("note version not found", None)
        }
        NoteReadUseCaseError::Query(NoteReadQueryError::Database(error)) => {
            map_persistence_error(error)
        }
        NoteReadUseCaseError::Query(NoteReadQueryError::InvalidData(message)) => {
            internal_error(message)
        }
    }
}
impl MgmtServer {
    pub(super) async fn list_recent_notes_inner(
        &self,
        params: ListRecentParams,
    ) -> Result<ListRecentNotesResult, McpError> {
        let limit = clamp_limit(params.limit);
        let overviews = self
            .dependencies
            .note_reads
            .list_note_overviews(NoteOverviewQuery::default())
            .await
            .map_err(note_read_error_to_mcp)?;
        let notes = overviews
            .into_iter()
            .take(limit as usize)
            .map(|snapshot| {
                let representative = snapshot.representative;
                NoteMeta {
                    note_id: representative.note.id,
                    title: representative.version.title,
                    status: representative.version.status,
                    created_by_kind: representative.created_by_kind,
                    updated_at: representative.note.updated_at,
                    is_current: snapshot
                        .current
                        .is_some_and(|current| current.id == representative.version.id),
                    unread_version_count: snapshot.unread_version_count,
                }
            })
            .collect();
        Ok(ListRecentNotesResult { notes })
    }

    pub(super) async fn list_recent_annotations_inner(
        &self,
        params: ListRecentParams,
    ) -> Result<ListRecentAnnotationsResult, McpError> {
        let limit = clamp_limit(params.limit);
        let rows = self
            .dependencies
            .annotation_reads
            .list_recent_annotations(limit)
            .await
            .map_err(|error| match error {
                AnnotationReadUseCaseError::Query(AnnotationReadQueryError::Database(error)) => {
                    map_persistence_error(error)
                }
                other => internal_error(format!("{other}")),
            })?;
        let annotations = rows
            .into_iter()
            .map(|row| AnnotationMeta {
                annotation_id: row.id,
                target_symbol: row.target_symbol,
                target_kind: row.target_kind,
                status: row.status,
                created_by_kind: row.created_by_kind,
                updated_at: row.updated_at,
            })
            .collect();
        Ok(ListRecentAnnotationsResult { annotations })
    }
}
