//! 管理 MCP の直近ノート・アノテーション一覧 tool。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use core_application::note::{NoteListQuery, NoteReadQueryError, NoteReadUseCaseError};
use gateway_postgres::entities::annotation;

use super::MgmtServer;
use super::dto::{
    AnnotationMeta, ListRecentAnnotationsResult, ListRecentNotesResult, ListRecentParams, NoteMeta,
};
use super::{clamp_limit, db_error, internal_error, invalid_params, map_app_error};

fn note_read_error_to_mcp(error: NoteReadUseCaseError) -> McpError {
    match error {
        NoteReadUseCaseError::NotFound(_) => McpError::resource_not_found("note not found", None),
        NoteReadUseCaseError::Forbidden(note_id) => invalid_params(format!(
            "forbidden: note {note_id} belongs to another strategy"
        )),
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
            map_app_error(error.into())
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
        let page = self
            .use_cases
            .note_reads()
            .list_notes(
                None,
                NoteListQuery {
                    strategy_id: Some(params.strategy_id),
                    limit: Some(limit),
                    ..NoteListQuery::default()
                },
            )
            .await
            .map_err(note_read_error_to_mcp)?;
        let notes = page
            .notes
            .into_iter()
            .map(|snapshot| {
                Ok(NoteMeta {
                    note_id: snapshot.note.id,
                    title: snapshot.version.title,
                    status: snapshot.version.status,
                    created_by_kind: snapshot.created_by_kind,
                    updated_at: snapshot.note.updated_at,
                })
            })
            .collect::<Result<Vec<_>, McpError>>()?;
        Ok(ListRecentNotesResult { notes })
    }

    pub(super) async fn list_recent_annotations_inner(
        &self,
        params: ListRecentParams,
    ) -> Result<ListRecentAnnotationsResult, McpError> {
        let limit = clamp_limit(params.limit);
        let rows = annotation::Entity::find()
            .filter(annotation::Column::StrategyId.eq(params.strategy_id))
            .order_by_desc(annotation::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::insert_test_note;
    use rmcp::handler::server::wrapper::{Json, Parameters};

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[backend_test_macros::database_test]
    async fn list_recent_notes_caps_by_limit(db: gateway_postgres::DatabaseHandle) {
        let strategy_id = insert_strategy(&db, "long").await;
        for i in 0..5 {
            insert_test_note(&db, strategy_id, &format!("note-{i}"), "body").await;
        }
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let Json(result) = server
            .list_recent_notes(Parameters(ListRecentParams {
                strategy_id,
                limit: Some(3),
            }))
            .await
            .expect("ok");
        assert_eq!(result.notes.len(), 3);
    }
}
