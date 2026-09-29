//! 戦略タスクの application use case への互換アダプター。

mod resume;
pub use resume::{ResumeTaskError, auto_resume_task, resume_task};

use uuid::Uuid;

pub use core_application::strategy_task::{
    DEADLINE_DURATION, DEFAULT_PURPOSE, GetTaskError, ListTasksError, SubmitTaskError,
    SubmittedTask, TaskListQuery, TaskSource, TaskStatusView,
};
pub use gateway_postgres::entities::sea_orm_active_enums::{
    StrategyTaskPhase, StrategyTaskStepStatus,
};

pub fn phase_str(phase: &StrategyTaskPhase) -> &'static str {
    match phase {
        StrategyTaskPhase::Pending => "pending",
        StrategyTaskPhase::Running => "running",
        StrategyTaskPhase::Completed => "completed",
        StrategyTaskPhase::Failed => "failed",
    }
}

impl From<TaskStatusView> for crate::models::StrategyTaskSummary {
    fn from(view: TaskStatusView) -> Self {
        Self {
            task_id: view.task_id,
            strategy_id: view.strategy_id,
            source: view.source,
            prompt: view.prompt,
            phase: view.phase.as_str().to_string(),
            error_summary: view.error_summary,
            created_at: view.created_at,
            updated_at: view.updated_at,
            purpose: view.purpose,
            as_of: view.as_of,
        }
    }
}

pub async fn submit_task<C>(
    db: &C,
    agent_client: &crate::agent_client::SharedAgentTaskClient,
    strategy_id: Uuid,
    prompt: &str,
    source: TaskSource,
    purpose: Option<String>,
) -> Result<SubmittedTask, SubmitTaskError>
where
    C: sea_orm::ConnectionTrait + Clone + Into<gateway_postgres::DatabaseHandle>,
{
    crate::services::use_cases::build_strategy_task_use_cases(db.clone())
        .submit_task(agent_client.as_ref(), strategy_id, prompt, source, purpose)
        .await
}

pub async fn list_tasks<C>(
    db: &C,
    strategy_id: Option<Uuid>,
    purpose: Option<String>,
) -> Result<Vec<TaskStatusView>, ListTasksError>
where
    C: sea_orm::ConnectionTrait + Clone + Into<gateway_postgres::DatabaseHandle>,
{
    crate::services::use_cases::build_strategy_task_use_cases(db.clone())
        .list(TaskListQuery {
            strategy_id,
            purpose,
        })
        .await
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::agent_client::{FakeAgentTaskClient, SharedAgentTaskClient};
    use crate::testing::insert_test_strategy;
    use gateway_postgres::entities::strategy_task;
    use rstest::rstest;
    use sea_orm::{EntityTrait, PaginatorTrait};

    #[rstest]
    #[case::mgmt_mcp(TaskSource::MgmtMcp, "mgmt-mcp")]
    #[case::frontend(TaskSource::Frontend, "frontend")]
    #[case::review(TaskSource::Review, "review")]
    fn task_source_as_str(#[case] source: TaskSource, #[case] expected: &str) {
        assert_eq!(source.as_str(), expected);
    }

    #[backend_test_macros::database_test]
    async fn submit_task_rejects_missing_purpose_before_inserting_task_row(
        db: gateway_postgres::DatabaseHandle,
    ) {
        let strategy_id = insert_test_strategy(&db, "s").await;
        let agent_client: SharedAgentTaskClient = Arc::new(FakeAgentTaskClient::new());

        let err = submit_task(
            &db,
            &agent_client,
            strategy_id,
            "prompt",
            TaskSource::Review,
            None,
        )
        .await
        .expect_err("missing agent_config should be rejected");
        assert!(matches!(err, SubmitTaskError::PurposeNotFound(p) if p == DEFAULT_PURPOSE));

        let task_count = strategy_task::Entity::find().count(&db).await.unwrap();
        assert_eq!(task_count, 0);
    }
}
