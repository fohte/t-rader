//! 管理 MCP の戦略一覧・タスク投入・タスク status tool。

use std::collections::HashMap;

use rmcp::ErrorData as McpError;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::agent_client::AgentTaskError;
use crate::entities::{annotation, note, strategy};
use crate::services::strategy_tasks::{
    self, SubmitTaskError, TaskSource, TaskStatusView, phase_str,
};

use super::dto::{
    GetStrategyTaskStatusParams, GetStrategyTaskStatusResult, ListStrategiesResult,
    StrategySummary, SubmitStrategyTaskParams, SubmitStrategyTaskResult,
};
use super::{MgmtServer, db_error, internal_error, invalid_params};

/// 指定エンティティの `status='unread'` 件数を strategy_id ごとに集約して返す。
/// `list_strategies` が note / annotation 双方に対し 1 クエリで未読件数を取るために使う。
async fn unread_counts_by_strategy<E, C>(
    db: &DatabaseConnection,
    strategy_id_col: C,
    status_col: C,
    id_col: C,
) -> Result<HashMap<Uuid, u64>, McpError>
where
    E: EntityTrait,
    C: ColumnTrait,
{
    let rows: Vec<(Uuid, i64)> = E::find()
        .select_only()
        .column(strategy_id_col)
        .column_as(id_col.count(), "unread_count")
        .filter(status_col.eq("unread"))
        .group_by(strategy_id_col)
        .into_tuple()
        .all(db)
        .await
        .map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|(sid, c)| (sid, c.max(0) as u64))
        .collect())
}

impl MgmtServer {
    pub(super) async fn list_strategies_inner(&self) -> Result<ListStrategiesResult, McpError> {
        let rows = strategy::Entity::find()
            .order_by_asc(strategy::Column::SortOrder)
            .order_by_asc(strategy::Column::CreatedAt)
            .all(&self.db)
            .await
            .map_err(db_error)?;

        let note_counts = unread_counts_by_strategy::<note::Entity, note::Column>(
            &self.db,
            note::Column::StrategyId,
            note::Column::Status,
            note::Column::Id,
        )
        .await?;
        let annotation_counts =
            unread_counts_by_strategy::<annotation::Entity, annotation::Column>(
                &self.db,
                annotation::Column::StrategyId,
                annotation::Column::Status,
                annotation::Column::Id,
            )
            .await?;

        let strategies = rows
            .into_iter()
            .map(|row| {
                let note_unread = note_counts.get(&row.id).copied().unwrap_or(0);
                let annotation_unread = annotation_counts.get(&row.id).copied().unwrap_or(0);
                StrategySummary {
                    strategy_id: row.id,
                    name: row.name,
                    updated_at: row.updated_at,
                    unread_card_count: note_unread + annotation_unread,
                }
            })
            .collect();
        Ok(ListStrategiesResult { strategies })
    }

    pub(super) async fn submit_strategy_task_inner(
        &self,
        params: SubmitStrategyTaskParams,
    ) -> Result<SubmitStrategyTaskResult, McpError> {
        let submitted = strategy_tasks::submit_task(
            &self.db,
            &self.agent_client,
            params.strategy_id,
            &params.prompt,
            TaskSource::MgmtMcp,
        )
        .await
        .map_err(map_submit_error)?;
        Ok(SubmitStrategyTaskResult {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        })
    }

    pub(super) async fn get_strategy_task_status_inner(
        &self,
        params: GetStrategyTaskStatusParams,
    ) -> Result<GetStrategyTaskStatusResult, McpError> {
        let view: TaskStatusView =
            strategy_tasks::get_task_by_a2a_task_id(&self.db, &params.a2a_task_id)
                .await
                .map_err(db_error)?
                .ok_or_else(|| McpError::resource_not_found("strategy task not found", None))?;
        Ok(GetStrategyTaskStatusResult {
            task_id: view.task_id,
            strategy_id: view.strategy_id,
            a2a_task_id: view.a2a_task_id,
            phase: phase_str(&view.phase).to_string(),
            error_summary: view.error_summary,
            result_text: view.result_text,
            updated_at: view.updated_at,
        })
    }
}

fn map_submit_error(err: SubmitTaskError) -> McpError {
    match err {
        SubmitTaskError::EmptyPrompt => invalid_params("prompt must not be empty"),
        SubmitTaskError::StrategyNotFound(id) => invalid_params(format!("strategy {id} not found")),
        SubmitTaskError::Database(db_err) => db_error(db_err),
        SubmitTaskError::AgentTask(agent_err) => map_agent_task_error(&agent_err),
    }
}

fn map_agent_task_error(err: &AgentTaskError) -> McpError {
    match err {
        AgentTaskError::NotConfigured => internal_error("agent task client is not configured"),
        AgentTaskError::NotFound(name) => {
            McpError::resource_not_found(format!("agent task not found: {name}"), None)
        }
        AgentTaskError::Api { status, message } => {
            internal_error(format!("agent task api error (status {status}): {message}"))
        }
        AgentTaskError::Network(msg) => internal_error(format!("agent task network error: {msg}")),
        AgentTaskError::Parse(msg) => internal_error(format!("agent task parse error: {msg}")),
        AgentTaskError::Init(msg) => internal_error(format!("agent task init error: {msg}")),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::{Json, Parameters};
    use sea_orm::ActiveModelTrait;
    use sea_orm::ActiveValue::Set;
    use sqlx::PgPool;

    use crate::agent_client::FakeAgentTaskClient;
    use crate::entities::sea_orm_active_enums::StrategyTaskPhase;
    use crate::entities::strategy_task;
    use crate::testing::create_test_db;

    use super::super::tests_common::{build_server, insert_strategy};

    use super::*;

    /// 管理 MCP 経由で投入されたタスクの `strategy_task.source` 値。
    const MGMT_TASK_SOURCE: &str = "mgmt-mcp";

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_inserts_row_and_submits_to_agent(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long-term").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_next_task_id("agent-task-1").await;
        let server = build_server(db.clone(), fake.clone());

        let Json(result) = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: " inspect 7203 ".into(),
            }))
            .await
            .expect("submit ok");

        assert_eq!(result.a2a_task_id, "agent-task-1");
        let task_id = result.task_id;

        // agent_client への投入は前後の空白を trim した prompt で 1 件だけ記録される
        let submitted: Vec<(uuid::Uuid, String)> = fake
            .submitted
            .lock()
            .await
            .iter()
            .map(|s| (s.strategy_id, s.prompt.clone()))
            .collect();
        assert_eq!(submitted, vec![(strategy_id, "inspect 7203".to_string())]);

        // strategy_task 行は running で 1 件、戻り値と一致する内容を持つ
        let rows: Vec<StrategyTaskRowSummary> = strategy_task::Entity::find()
            .all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(StrategyTaskRowSummary::from_model)
            .collect();
        assert_eq!(
            rows,
            vec![StrategyTaskRowSummary {
                task_id,
                strategy_id,
                a2a_task_id: Some("agent-task-1".to_string()),
                source: MGMT_TASK_SOURCE.to_string(),
                prompt: "inspect 7203".to_string(),
                phase: StrategyTaskPhase::Running,
                error_summary: None,
            }],
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    struct StrategyTaskRowSummary {
        task_id: Uuid,
        strategy_id: Uuid,
        a2a_task_id: Option<String>,
        source: String,
        prompt: String,
        phase: StrategyTaskPhase,
        error_summary: Option<String>,
    }

    impl StrategyTaskRowSummary {
        fn from_model(m: strategy_task::Model) -> Self {
            Self {
                task_id: m.task_id,
                strategy_id: m.strategy_id,
                a2a_task_id: m.a2a_task_id,
                source: m.source,
                prompt: m.prompt,
                phase: m.phase,
                error_summary: m.error_summary,
            }
        }
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_rejects_unknown_strategy(pool: PgPool) {
        let db = create_test_db(pool).await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let server = build_server(db, fake);

        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id: Uuid::new_v4(),
                prompt: "x".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_rejects_empty_prompt(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "   ".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[sqlx::test(migrations = false)]
    async fn submit_strategy_task_persists_failure_on_agent_error(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        fake.set_submit_error(AgentTaskError::Api {
            status: 500,
            message: "boom".into(),
        })
        .await;
        let server = build_server(db.clone(), fake);

        let err = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "x".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);

        let rows = strategy_task::Entity::find().all(&db).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].phase, StrategyTaskPhase::Failed);
        assert_eq!(
            rows[0].error_summary.as_deref(),
            Some("agent task submission failed: agent task api error (status 500): boom"),
        );
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_status_returns_row(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "x").await;
        let fake = Arc::new(FakeAgentTaskClient::new());
        let server = build_server(db.clone(), fake);

        let Json(submitted) = server
            .submit_strategy_task(Parameters(SubmitStrategyTaskParams {
                strategy_id,
                prompt: "p".into(),
            }))
            .await
            .expect("submit");

        let Json(status) = server
            .get_strategy_task_status(Parameters(GetStrategyTaskStatusParams {
                a2a_task_id: submitted.a2a_task_id.clone(),
            }))
            .await
            .expect("ok");
        assert_eq!(status.task_id, submitted.task_id);
        assert_eq!(status.strategy_id, strategy_id);
        assert_eq!(status.phase, "running");
        assert!(status.error_summary.is_none());
        assert!(status.result_text.is_none());
    }

    #[sqlx::test(migrations = false)]
    async fn get_strategy_task_status_not_found(pool: PgPool) {
        let db = create_test_db(pool).await;
        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let err = server
            .get_strategy_task_status(Parameters(GetStrategyTaskStatusParams {
                a2a_task_id: "nonexistent".into(),
            }))
            .await
            .err()
            .unwrap_or_else(|| panic!("expected error"));
        assert_eq!(err.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[sqlx::test(migrations = false)]
    async fn list_strategies_counts_unread_cards(pool: PgPool) {
        let db = create_test_db(pool).await;
        let strategy_id = insert_strategy(&db, "long").await;

        // unread ノート 2 件、approved ノート 1 件 → unread だけカウント
        for (title, status) in [("a", "unread"), ("b", "unread"), ("c", "approved")] {
            note::ActiveModel {
                id: Set(Uuid::new_v4()),
                strategy_id: Set(strategy_id),
                title: Set(title.to_string()),
                body_md: Set("body".to_string()),
                frontmatter_json: Set(serde_json::json!({})),
                type_tag: Set(None),
                status: Set(status.to_string()),
                trigger: Set(None),
                trigger_label: Set(None),
                created_by_kind: Set("human".to_string()),
                created_at: sea_orm::ActiveValue::NotSet,
                updated_at: sea_orm::ActiveValue::NotSet,
                graphs_json: Set(serde_json::json!([])),
                execution_id: Set(None),
            }
            .insert(&db)
            .await
            .unwrap();
        }
        // unread アノテーション 1 件
        annotation::ActiveModel {
            id: Set(Uuid::new_v4()),
            strategy_id: Set(strategy_id),
            target_symbol: Set("7203".into()),
            target_kind: Set("signal".into()),
            timestamp: Set(chrono::Utc::now().fixed_offset()),
            price: Set(None),
            text: Set("note".into()),
            status: Set("unread".into()),
            linked_note_id: Set(None),
            created_by_kind: Set("llm".into()),
            created_at: sea_orm::ActiveValue::NotSet,
            updated_at: sea_orm::ActiveValue::NotSet,
        }
        .insert(&db)
        .await
        .unwrap();

        let server = build_server(db, Arc::new(FakeAgentTaskClient::new()));
        let Json(result) = server.list_strategies().await.expect("ok");
        assert_eq!(result.strategies.len(), 1);
        assert_eq!(result.strategies[0].strategy_id, strategy_id);
        assert_eq!(result.strategies[0].unread_card_count, 3);
    }
}
