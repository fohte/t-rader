//! 管理 MCP の直近ノート・アノテーション一覧 tool。

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::entities::{annotation, note};

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
            .order_by_desc(note::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(db_error)?;
        let notes = rows
            .into_iter()
            .map(|row| NoteMeta {
                note_id: row.id,
                title: row.title,
                status: row.status,
                created_by_kind: row.created_by_kind,
                updated_at: row.updated_at,
            })
            .collect();
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
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::testing::create_test_db;

    use super::super::tests_common::{build_server, insert_strategy};
    use super::*;

    #[sqlx::test(migrations = false)]
    async fn list_recent_notes_caps_by_limit(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;
        for i in 0..5 {
            note::ActiveModel {
                id: Set(Uuid::new_v4()),
                strategy_id: Set(strategy_id),
                title: Set(format!("note-{i}")),
                body_md: Set("body".into()),
                frontmatter_json: Set(serde_json::json!({})),
                type_tag: Set(None),
                status: Set("unread".into()),
                trigger: Set(None),
                trigger_label: Set(None),
                created_by_kind: Set("human".into()),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
                graphs_json: Set(serde_json::json!([])),
            }
            .insert(&db)
            .await
            .unwrap();
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
