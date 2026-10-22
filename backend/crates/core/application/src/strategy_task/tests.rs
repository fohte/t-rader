use std::sync::Arc;

use uuid::Uuid;

use crate::agent_task_client::{AgentTaskError, FakeAgentTaskClient};
use crate::execution_budget::test_support::FakeExecutionBudgetRepository;
use crate::strategy_task::test_support::{FakeStrategyTaskRepository, agent_graph_with_timeouts};
use crate::strategy_task::{
    ResumeTaskError, StrategyTask, StrategyTaskPhase, StrategyTaskUseCases, SubmitTaskError,
    TaskSource,
};
use crate::unit_of_work::FakeUnitOfWork;
use rstest::rstest;

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Submitted,
    AgentNetworkError(String),
    OtherError(String),
    PurposeNotFound(String),
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
        deadline_at: now + chrono::Duration::minutes(15),
        purpose: Some("explore".to_string()),
        as_of: Some(now),
        context_kind: None,
        context_id: None,
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
#[case::event(TaskSource::Event, "event")]
fn task_source_as_str(#[case] source: TaskSource, #[case] expected: &str) {
    assert_eq!(source.as_str(), expected);
}

#[tokio::test]
async fn submit_rejects_missing_default_purpose_before_inserting_a_task() {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeStrategyTaskRepository::without_agent_config(None));
    let agent_client = FakeAgentTaskClient::new();
    let use_cases = StrategyTaskUseCases::new(
        unit_of_work,
        repository.clone(),
        Arc::new(FakeExecutionBudgetRepository::default()),
    );

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
    let task_inserted = repository.state.lock().await.task.is_some();
    let submitted_count = agent_client.submitted.lock().await.len();

    assert_eq!(
        (result, task_inserted, submitted_count),
        (Outcome::PurposeNotFound("default".to_string()), false, 0),
    );
}

#[rstest]
#[case::default_timeout(None, None, 15)]
#[case::phase_timeout_only(None, Some(42), 15)]
#[case::configured_timeout(Some(42), None, 42)]
#[tokio::test]
async fn submit_sets_deadline_from_agent_graph(
    #[case] top_level_timeout_minutes: Option<u32>,
    #[case] phase_timeout_minutes: Option<u32>,
    #[case] expected_minutes: i64,
) {
    let agent_graph = agent_graph_with_timeouts(top_level_timeout_minutes, phase_timeout_minutes);

    assert_eq!(
        submit_deadline_for_agent_graph(&agent_graph).await,
        (
            true,
            Some((chrono::Duration::minutes(expected_minutes), true)),
        ),
    );
}

#[tokio::test]
async fn submit_uses_default_deadline_when_agent_graph_cannot_be_parsed() {
    assert_eq!(
        submit_deadline_for_agent_graph("phases: [").await,
        (true, Some((chrono::Duration::minutes(15), true))),
    );
}

#[tokio::test]
async fn submit_initializes_execution_budget_for_the_new_task() {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeStrategyTaskRepository::new(None, Vec::new()));
    let execution_budget = Arc::new(FakeExecutionBudgetRepository::default());
    let agent_client = FakeAgentTaskClient::new();
    let use_cases =
        StrategyTaskUseCases::new(unit_of_work, repository.clone(), execution_budget.clone());

    let submitted = use_cases
        .submit_task(
            &agent_client,
            Uuid::nil(),
            "synthetic prompt",
            TaskSource::Review,
            Some("fictional-purpose".to_string()),
        )
        .await
        .is_ok();
    let task = repository
        .state
        .lock()
        .await
        .task
        .clone()
        .expect("submitted task is stored");
    let budget_deadline = execution_budget.initialized_deadline(task.task_id).await;
    let request = agent_client
        .submitted
        .lock()
        .await
        .first()
        .map(|request| (request.task_id, request.deadline_at));

    assert_eq!(
        (submitted, budget_deadline, request),
        (
            true,
            Some(task.deadline_at),
            Some((task.task_id, task.deadline_at)),
        ),
    );
}

async fn submit_deadline_for_agent_graph(
    agent_graph: &str,
) -> (bool, Option<(chrono::Duration, bool)>) {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeStrategyTaskRepository::new(None, Vec::new()));
    repository.set_agent_graph(agent_graph).await;
    let execution_budget = Arc::new(FakeExecutionBudgetRepository::default());
    let agent_client = FakeAgentTaskClient::new();
    let use_cases =
        StrategyTaskUseCases::new(unit_of_work, repository.clone(), execution_budget.clone());

    let submitted = use_cases
        .submit_task(
            &agent_client,
            Uuid::nil(),
            "prompt",
            TaskSource::Review,
            Some("fictional-purpose".to_string()),
        )
        .await
        .is_ok();

    let task = repository.state.lock().await.task.clone();
    let submitted_deadline = agent_client
        .submitted
        .lock()
        .await
        .first()
        .map(|request| request.deadline_at);

    (
        submitted,
        task.zip(submitted_deadline)
            .map(|(task, submitted_deadline)| {
                (
                    task.deadline_at - task.created_at,
                    task.deadline_at == submitted_deadline,
                )
            }),
    )
}

#[tokio::test]
async fn submit_returns_agent_error_when_marking_failed_also_fails() {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeStrategyTaskRepository::failing_failed_update(None));
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_submit_error(AgentTaskError::Network(
            "submission unavailable".to_string(),
        ))
        .await;
    let use_cases = StrategyTaskUseCases::new(
        unit_of_work.clone(),
        repository.clone(),
        Arc::new(FakeExecutionBudgetRepository::default()),
    );

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
    let repository = Arc::new(FakeStrategyTaskRepository::failing_failed_update(Some(
        resumable_task(),
    )));
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_submit_error(AgentTaskError::Network("resume unavailable".to_string()))
        .await;
    let use_cases = StrategyTaskUseCases::new(
        unit_of_work.clone(),
        repository.clone(),
        Arc::new(FakeExecutionBudgetRepository::default()),
    );

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
