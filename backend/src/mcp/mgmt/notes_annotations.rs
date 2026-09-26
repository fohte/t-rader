//! 管理 MCP の直近ノート・アノテーション一覧 tool。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::entities::{annotation, note};
use crate::services::note_versions::{
    current_note_ids, find_current_versions, find_initial_created_by_kind,
};

use super::MgmtServer;
use super::dto::{
    AnnotationMeta, ListRecentAnnotationsResult, ListRecentNotesResult, ListRecentParams, NoteMeta,
};
use super::{clamp_limit, db_error};

impl MgmtServer {
    pub(super) async fn list_recent_notes_inner(
        &self,
        params: ListRecentParams,
    ) -> Result<ListRecentNotesResult, McpError> {
        let limit = clamp_limit(params.limit);
        let rows = note::Entity::find()
            .filter(note::Column::StrategyId.eq(params.strategy_id))
            .filter(note::Column::Id.in_subquery(current_note_ids()))
            .order_by_desc(note::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        let versions =
            find_current_versions(&self.db, &rows.iter().map(|row| row.id).collect::<Vec<_>>())
                .await
                .map_err(db_error)?;
        let creators = find_initial_created_by_kind(
            &self.db,
            &rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        )
        .await
        .map_err(db_error)?;
        let notes = rows
            .into_iter()
            .map(|row| {
                let version = versions.get(&row.id).ok_or_else(|| {
                    db_error(sea_orm::DbErr::Custom(format!(
                        "note {} has no current version",
                        row.id
                    )))
                })?;
                let created_by_kind = creators.get(&row.id).cloned().ok_or_else(|| {
                    db_error(sea_orm::DbErr::Custom(format!(
                        "note {} has no initial version",
                        row.id
                    )))
                })?;
                Ok(NoteMeta {
                    note_id: row.id,
                    title: version.title.clone(),
                    status: version.status.clone(),
                    created_by_kind,
                    updated_at: row.updated_at,
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

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sqlx::PgPool;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::{create_test_db, insert_test_note};

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[backend_test_macros::database_test]
    async fn list_recent_notes_caps_by_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
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
