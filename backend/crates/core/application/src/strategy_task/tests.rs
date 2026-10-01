use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::agent_task_client::{AgentTaskError, FakeAgentTaskClient};
use crate::persistence::PersistenceError;
use crate::strategy_task::repository::{StrategyTaskRepository, StrategyTaskRepositoryError};
use crate::strategy_task::{
    ResumeTaskError, StrategyTask, StrategyTaskPhase, StrategyTaskStep, StrategyTaskUpdate,
    StrategyTaskUseCases, SubmitTaskError, TaskListQuery, TaskSource,
};
use crate::unit_of_work::{FakeUnitOfWork, UnitOfWorkTransaction};
use rstest::rstest;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Submitted,
    AgentNetworkError(String),
    OtherError(String),
    PurposeNotFound(String),
}

struct FailingFailedUpdateRepository {
    task: Mutex<Option<StrategyTask>>,
    update_phases: Mutex<Vec<Option<StrategyTaskPhase>>>,
    agent_config_exists: bool,
}

impl FailingFailedUpdateRepository {
    fn new(task: Option<StrategyTask>) -> Self {
        Self {
            task: Mutex::new(task),
            update_phases: Mutex::new(Vec::new()),
            agent_config_exists: true,
        }
    }

    fn without_agent_config(task: Option<StrategyTask>) -> Self {
        Self {
            task: Mutex::new(task),
            update_phases: Mutex::new(Vec::new()),
            agent_config_exists: false,
        }
    }
}

#[async_trait]
impl StrategyTaskRepository for FailingFailedUpdateRepository {
    async fn strategy_exists(
        &self,
        _strategy_id: Uuid,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        Ok(true)
    }

    async fn agent_config_exists(
        &self,
        _purpose: &str,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        Ok(self.agent_config_exists)
    }

    async fn insert(
        &self,
        _transaction: &UnitOfWorkTransaction,
        task: StrategyTask,
    ) -> Result<(), StrategyTaskRepositoryError> {
        *self.task.lock().await = Some(task);
        Ok(())
    }

    async fn update(
        &self,
        _transaction: &UnitOfWorkTransaction,
        task: StrategyTaskUpdate,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        self.update_phases.lock().await.push(task.phase);
        if task.phase == Some(StrategyTaskPhase::Failed) {
            Err(StrategyTaskRepositoryError::Database(
                PersistenceError::Database("simulated update failure".to_string()),
            ))
        } else {
            Ok(true)
        }
    }

    async fn apply_status_and_steps(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _task_id: Uuid,
        _task_update: Option<StrategyTaskUpdate>,
        _steps: Option<serde_json::Value>,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        Ok(false)
    }

    async fn find_by_id(
        &self,
        task_id: Uuid,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
        Ok(self
            .task
            .lock()
            .await
            .as_ref()
            .filter(|task| task.task_id == task_id)
            .cloned())
    }

    async fn find_by_a2a_task_id(
        &self,
        a2a_task_id: &str,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
        Ok(self
            .task
            .lock()
            .await
            .as_ref()
            .filter(|task| task.a2a_task_id.as_deref() == Some(a2a_task_id))
            .cloned())
    }

    async fn list(
        &self,
        _query: TaskListQuery,
    ) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
        Ok(Vec::new())
    }

    async fn list_in_flight(&self) -> Result<Vec<StrategyTask>, StrategyTaskRepositoryError> {
        Ok(Vec::new())
    }

    async fn list_steps(
        &self,
        _task_id: Uuid,
    ) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError> {
        Ok(Vec::new())
    }

    async fn claim_resumable(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _task_id: Uuid,
        _now: chrono::DateTime<chrono::FixedOffset>,
        _mark_auto_resumed: bool,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        Ok(true)
    }
}

fn resumable_task() -> StrategyTask {
    let now = chrono::Utc::now().fixed_offset();
    StrategyTask {
        task_id: Uuid::nil(),
        strategy_id: Uuid::nil(),
        a2a_task_id: Some("previous-agent-task".to_string()),
        source: TaskSource::Review.as_str().to_string(),
        prompt: "prompt".to_string(),
        phase: StrategyTaskPhase::Failed,
        error_summary: Some("previous failure".to_string()),
        result_text: None,
        deadline_at: now,
        purpose: Some("explore".to_string()),
        as_of: Some(now),
        auto_resumed_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn submit_outcome(result: Result<super::SubmittedTask, SubmitTaskError>) -> Outcome {
    match result {
        Ok(_) => Outcome::Submitted,
        Err(SubmitTaskError::AgentTask(AgentTaskError::Network(error))) => {
            Outcome::AgentNetworkError(error)
        }
        Err(SubmitTaskError::PurposeNotFound(purpose)) => Outcome::PurposeNotFound(purpose),
        Err(error) => Outcome::OtherError(error.to_string()),
    }
}

fn resume_outcome(result: Result<super::SubmittedTask, ResumeTaskError>) -> Outcome {
    match result {
        Ok(_) => Outcome::Submitted,
        Err(ResumeTaskError::AgentTask(AgentTaskError::Network(error))) => {
            Outcome::AgentNetworkError(error)
        }
        Err(error) => Outcome::OtherError(error.to_string()),
    }
}

#[rstest]
#[case::mgmt_mcp(TaskSource::MgmtMcp, "mgmt-mcp")]
#[case::frontend(TaskSource::Frontend, "frontend")]
#[case::cron(TaskSource::Cron, "cron")]
#[case::hook(TaskSource::Hook, "hook")]
#[case::review(TaskSource::Review, "review")]
fn task_source_as_str(#[case] source: TaskSource, #[case] expected: &str) {
    assert_eq!(source.as_str(), expected);
}

#[tokio::test]
async fn submit_rejects_missing_default_purpose_before_inserting_a_task() {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FailingFailedUpdateRepository::without_agent_config(None));
    let agent_client = FakeAgentTaskClient::new();
    let use_cases = StrategyTaskUseCases::new(unit_of_work, repository.clone());

    let result = submit_outcome(
        use_cases
            .submit_task(
                &agent_client,
                Uuid::nil(),
                "prompt",
                TaskSource::Review,
                None,
            )
            .await,
    );
    let task_inserted = repository.task.lock().await.is_some();
    let submitted_count = agent_client.submitted.lock().await.len();

    assert_eq!(
        (result, task_inserted, submitted_count),
        (Outcome::PurposeNotFound("default".to_string()), false, 0),
    );
}

#[tokio::test]
async fn submit_returns_agent_error_when_marking_failed_also_fails() {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FailingFailedUpdateRepository::new(None));
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_submit_error(AgentTaskError::Network(
            "submission unavailable".to_string(),
        ))
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = submit_outcome(
        use_cases
            .submit_task(
                &agent_client,
                Uuid::nil(),
                "prompt",
                TaskSource::Review,
                Some("explore".to_string()),
            )
            .await,
    );
    let update_phases = repository.update_phases.lock().await.clone();
    let begun = unit_of_work.begun.lock().await.len();
    let committed = unit_of_work.committed.lock().await.len();

    assert_eq!(
        (result, update_phases, begun, committed),
        (
            Outcome::AgentNetworkError("submission unavailable".to_string()),
            vec![Some(StrategyTaskPhase::Failed)],
            2,
            1,
        ),
    );
}

#[tokio::test]
async fn resume_returns_agent_error_when_marking_failed_also_fails() {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FailingFailedUpdateRepository::new(Some(resumable_task())));
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_submit_error(AgentTaskError::Network("resume unavailable".to_string()))
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = resume_outcome(use_cases.resume(&agent_client, Uuid::nil()).await);
    let update_phases = repository.update_phases.lock().await.clone();
    let begun = unit_of_work.begun.lock().await.len();
    let committed = unit_of_work.committed.lock().await.len();

    assert_eq!(
        (result, update_phases, begun, committed),
        (
            Outcome::AgentNetworkError("resume unavailable".to_string()),
            vec![Some(StrategyTaskPhase::Failed)],
            2,
            1,
        ),
    );
}
