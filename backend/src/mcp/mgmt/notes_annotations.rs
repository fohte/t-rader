//! 管理 MCP の直近ノート・アノテーション一覧 tool。

use rmcp::ErrorData as McpError;

use core_application::annotation::{AnnotationReadQueryError, AnnotationReadUseCaseError};
use core_application::note::{NoteListQuery, NoteReadQueryError, NoteReadUseCaseError};

use super::MgmtServer;
use super::dto::{
    AnnotationMeta, ListRecentAnnotationsResult, ListRecentNotesResult, ListRecentParams, NoteMeta,
};
use super::{clamp_limit, internal_error, invalid_params, map_persistence_error};

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
        let page = self
            .dependencies
            .note_reads
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
        let rows = self
            .dependencies
            .annotation_reads
            .list_recent_annotations(params.strategy_id, limit)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::insert_test_note;
    use chrono::{DateTime, FixedOffset};
    use gateway_postgres::entities::annotation;
    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use uuid::Uuid;

    use super::super::dto::{AnnotationMeta, ListRecentAnnotationsResult};
    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    fn test_timestamp(value: &str) -> DateTime<FixedOffset> {
        value.parse().expect("timestamp")
    }

    async fn seed_annotation(
        db: &gateway_postgres::DatabaseHandle,
        strategy_id: Uuid,
        target_symbol: &str,
        target_kind: &str,
        status: &str,
        created_by_kind: &str,
        updated_at: DateTime<FixedOffset>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        annotation::ActiveModel {
            id: Set(id),
            strategy_id: Set(Some(strategy_id)),
            target_symbol: Set(target_symbol.into()),
            target_kind: Set(target_kind.into()),
            timestamp: Set(updated_at),
            price: Set(None),
            text: Set("sample text".into()),
            status: Set(status.into()),
            linked_note_id: Set(None),
            created_by_kind: Set(created_by_kind.into()),
            created_at: Set(updated_at),
            updated_at: Set(updated_at),
            execution_step_id: Set(None),
            execution_task_id: Set(None),
        }
        .insert(db)
        .await
        .expect("insert annotation");
        id
    }

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

    #[backend_test_macros::database_test]
    async fn list_recent_annotations_filters_strategy_and_orders_by_updated_at(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_strategy(&db, "sample-strategy").await;
        let foreign_strategy_id = insert_strategy(&db, "foreign-strategy").await;
        let older_updated_at = test_timestamp("2026-07-01T00:00:00Z");
        let newer_updated_at = test_timestamp("2026-07-02T00:00:00Z");
        let foreign_updated_at = test_timestamp("2026-07-03T00:00:00Z");
        let older_id = seed_annotation(
            &db,
            strategy_id,
            "SAMPLE-A",
            "sample-kind-a",
            "unread",
            "human",
            older_updated_at,
        )
        .await;
        let newer_id = seed_annotation(
            &db,
            strategy_id,
            "SAMPLE-B",
            "sample-kind-b",
            "approved",
            "llm",
            newer_updated_at,
        )
        .await;
        seed_annotation(
            &db,
            foreign_strategy_id,
            "FOREIGN",
            "foreign-kind",
            "rejected",
            "human",
            foreign_updated_at,
        )
        .await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));

        let Json(result) = server
            .list_recent_annotations(Parameters(ListRecentParams {
                strategy_id,
                limit: Some(2),
            }))
            .await
            .expect("list recent annotations");

        assert_eq!(
            serde_json::to_value(result).expect("serialize result"),
            serde_json::to_value(ListRecentAnnotationsResult {
                annotations: vec![
                    AnnotationMeta {
                        annotation_id: newer_id,
                        target_symbol: "SAMPLE-B".into(),
                        target_kind: "sample-kind-b".into(),
                        status: "approved".into(),
                        created_by_kind: "llm".into(),
                        updated_at: newer_updated_at,
                    },
                    AnnotationMeta {
                        annotation_id: older_id,
                        target_symbol: "SAMPLE-A".into(),
                        target_kind: "sample-kind-a".into(),
                        status: "unread".into(),
                        created_by_kind: "human".into(),
                        updated_at: older_updated_at,
                    },
                ],
            })
            .expect("serialize expected result"),
        );
    }
}
