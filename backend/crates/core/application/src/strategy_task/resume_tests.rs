use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use rstest::rstest;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::agent_task_client::{AgentTaskError, FakeAgentTaskClient, SubmitAgentTask};
use crate::strategy_task::repository::{StrategyTaskRepository, StrategyTaskRepositoryError};
use crate::strategy_task::{
    ResumeTaskError, StrategyTask, StrategyTaskPhase, StrategyTaskStep, StrategyTaskStepStatus,
    StrategyTaskUpdate, StrategyTaskUseCases, TaskListQuery,
};
use crate::unit_of_work::{FakeUnitOfWork, UnitOfWorkTransaction};

#[derive(Default)]
struct RepositoryState {
    task: Option<StrategyTask>,
    steps: Vec<StrategyTaskStep>,
}

struct FakeStrategyTaskRepository {
    state: Mutex<RepositoryState>,
}

impl FakeStrategyTaskRepository {
    fn new(task: Option<StrategyTask>, steps: Vec<StrategyTaskStep>) -> Self {
        Self {
            state: Mutex::new(RepositoryState { task, steps }),
        }
    }

    async fn set_phase(&self, phase: StrategyTaskPhase) {
        if let Some(task) = self.state.lock().await.task.as_mut() {
            task.phase = phase;
        }
    }
}

#[async_trait]
impl StrategyTaskRepository for FakeStrategyTaskRepository {
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
        Ok(true)
    }

    async fn insert(
        &self,
        _transaction: &UnitOfWorkTransaction,
        task: StrategyTask,
    ) -> Result<(), StrategyTaskRepositoryError> {
        self.state.lock().await.task = Some(task);
        Ok(())
    }

    async fn update(
        &self,
        _transaction: &UnitOfWorkTransaction,
        update: StrategyTaskUpdate,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        let mut state = self.state.lock().await;
        let Some(task) = state
            .task
            .as_mut()
            .filter(|task| task.task_id == update.task_id)
        else {
            return Ok(false);
        };
        if let Some(a2a_task_id) = update.a2a_task_id {
            task.a2a_task_id = a2a_task_id;
        }
        if let Some(phase) = update.phase {
            task.phase = phase;
        }
        if let Some(error_summary) = update.error_summary {
            task.error_summary = error_summary;
        }
        if let Some(result_text) = update.result_text {
            task.result_text = result_text;
        }
        if let Some(deadline_at) = update.deadline_at {
            task.deadline_at = deadline_at;
        }
        if let Some(auto_resumed_at) = update.auto_resumed_at {
            task.auto_resumed_at = auto_resumed_at;
        }
        task.updated_at = update.updated_at;
        Ok(true)
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
            .state
            .lock()
            .await
            .task
            .as_ref()
            .filter(|task| task.task_id == task_id)
            .cloned())
    }

    async fn find_by_a2a_task_id(
        &self,
        a2a_task_id: &str,
    ) -> Result<Option<StrategyTask>, StrategyTaskRepositoryError> {
        Ok(self
            .state
            .lock()
            .await
            .task
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
        task_id: Uuid,
    ) -> Result<Vec<StrategyTaskStep>, StrategyTaskRepositoryError> {
        Ok(self
            .state
            .lock()
            .await
            .steps
            .iter()
            .filter(|step| step.task_id == task_id)
            .cloned()
            .collect())
    }

    async fn claim_resumable(
        &self,
        _transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        now: DateTime<FixedOffset>,
        mark_auto_resumed: bool,
    ) -> Result<bool, StrategyTaskRepositoryError> {
        let mut state = self.state.lock().await;
        let has_failed_step = state
            .steps
            .iter()
            .any(|step| step.task_id == task_id && step.status == StrategyTaskStepStatus::Failed);
        let Some(task) = state.task.as_mut().filter(|task| task.task_id == task_id) else {
            return Ok(false);
        };
        let resumable = task.phase == StrategyTaskPhase::Failed
            || (task.phase == StrategyTaskPhase::Completed && has_failed_step);
        let can_claim = resumable && (!mark_auto_resumed || task.auto_resumed_at.is_none());
        if can_claim {
            task.phase = StrategyTaskPhase::Running;
            task.updated_at = now;
            if mark_auto_resumed {
                task.auto_resumed_at = Some(now);
            }
        }
        Ok(can_claim)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ResumeResult {
    Submitted { task_id: Uuid, a2a_task_id: String },
    NotFound(Uuid),
    NotResumable { task_id: Uuid, phase: String },
    AgentTaskFailure(String),
    OtherFailure(String),
}

#[derive(Debug, PartialEq)]
struct RequestView {
    strategy_id: Uuid,
    prompt: String,
    purpose: Option<String>,
    resume_steps: Option<Vec<serde_json::Value>>,
    as_of: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, PartialEq)]
struct TaskView {
    phase: StrategyTaskPhase,
    a2a_task_id: Option<String>,
    error_summary: Option<String>,
    result_text: Option<String>,
    auto_resumed_at_is_set: bool,
    as_of: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, PartialEq)]
struct ResumeObservation {
    result: ResumeResult,
    requests: Vec<RequestView>,
    task: Option<TaskView>,
    begun_transactions: usize,
    committed_transactions: usize,
}

fn summarize_result(
    result: Result<crate::strategy_task::SubmittedTask, ResumeTaskError>,
) -> ResumeResult {
    match result {
        Ok(submitted) => ResumeResult::Submitted {
            task_id: submitted.task_id,
            a2a_task_id: submitted.a2a_task_id,
        },
        Err(ResumeTaskError::NotFound(task_id)) => ResumeResult::NotFound(task_id),
        Err(ResumeTaskError::NotResumable(task_id, phase)) => ResumeResult::NotResumable {
            task_id,
            phase: phase.to_string(),
        },
        Err(ResumeTaskError::AgentTask(error)) => ResumeResult::AgentTaskFailure(error.to_string()),
        Err(error) => ResumeResult::OtherFailure(error.to_string()),
    }
}

async fn observe(
    result: ResumeResult,
    repository: &FakeStrategyTaskRepository,
    agent_client: &FakeAgentTaskClient,
    unit_of_work: &FakeUnitOfWork,
) -> ResumeObservation {
    let requests = agent_client
        .submitted
        .lock()
        .await
        .iter()
        .map(|request: &SubmitAgentTask| RequestView {
            strategy_id: request.strategy_id,
            prompt: request.prompt.clone(),
            purpose: request.purpose.clone(),
            resume_steps: request.resume_steps.clone(),
            as_of: request.as_of,
        })
        .collect();
    let task = repository
        .state
        .lock()
        .await
        .task
        .as_ref()
        .map(|task| TaskView {
            phase: task.phase,
            a2a_task_id: task.a2a_task_id.clone(),
            error_summary: task.error_summary.clone(),
            result_text: task.result_text.clone(),
            auto_resumed_at_is_set: task.auto_resumed_at.is_some(),
            as_of: task.as_of,
        });

    ResumeObservation {
        result,
        requests,
        task,
        begun_transactions: unit_of_work.begun.lock().await.len(),
        committed_transactions: unit_of_work.committed.lock().await.len(),
    }
}

fn fixed_time() -> DateTime<FixedOffset> {
    (DateTime::<chrono::Utc>::UNIX_EPOCH + chrono::Duration::days(20_454)).fixed_offset()
}

fn task(phase: StrategyTaskPhase, auto_resumed_at: Option<DateTime<FixedOffset>>) -> StrategyTask {
    let now = fixed_time();
    StrategyTask {
        task_id: Uuid::from_u128(3),
        strategy_id: Uuid::from_u128(4),
        a2a_task_id: None,
        source: "review".to_string(),
        prompt: "prompt".to_string(),
        phase,
        error_summary: Some("previous failure".to_string()),
        result_text: Some("stale result".to_string()),
        deadline_at: now,
        purpose: Some("fictional-purpose".to_string()),
        as_of: Some(now),
        auto_resumed_at,
        created_at: now,
        updated_at: now,
    }
}

fn step(
    task_id: Uuid,
    execution_step_id: Uuid,
    phase_key: &str,
    status: StrategyTaskStepStatus,
    seq: i64,
) -> StrategyTaskStep {
    let now = fixed_time();
    StrategyTaskStep {
        execution_step_id,
        task_id,
        phase_key: phase_key.to_string(),
        label: phase_key.to_string(),
        model: "fictional-model".to_string(),
        status,
        item: None,
        item_label: None,
        output: None,
        started_at: now,
        finished_at: None,
        trace_id: "trace-placeholder".to_string(),
        span_id: "span-placeholder".to_string(),
        error: None,
        seq,
    }
}

#[rstest]
#[case::running_without_steps(StrategyTaskPhase::Running, None)]
#[case::running_with_failed_step(StrategyTaskPhase::Running, Some(StrategyTaskStepStatus::Failed))]
#[case::pending_with_failed_step(StrategyTaskPhase::Pending, Some(StrategyTaskStepStatus::Failed))]
#[case::completed_without_steps(StrategyTaskPhase::Completed, None)]
#[case::completed_with_only_completed_steps(
    StrategyTaskPhase::Completed,
    Some(StrategyTaskStepStatus::Completed)
)]
#[tokio::test]
async fn resume_rejects_task_when_repository_does_not_claim_it(
    #[case] phase: StrategyTaskPhase,
    #[case] step_status: Option<StrategyTaskStepStatus>,
) {
    let task = task(phase, None);
    let task_id = task.task_id;
    let steps = step_status
        .map(|status| vec![step(task_id, Uuid::from_u128(5), "phase", status, 1)])
        .unwrap_or_default();
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), steps));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = summarize_result(use_cases.resume(&agent_client, task_id).await);
    let actual = observe(result, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        actual,
        ResumeObservation {
            result: ResumeResult::NotResumable {
                task_id,
                phase: phase.as_str().to_string(),
            },
            requests: Vec::new(),
            task: Some(TaskView {
                phase,
                a2a_task_id: None,
                error_summary: Some("previous failure".to_string()),
                result_text: Some("stale result".to_string()),
                auto_resumed_at_is_set: false,
                as_of: Some(fixed_time()),
            }),
            begun_transactions: 1,
            committed_transactions: 1,
        },
    );
}

#[tokio::test]
async fn resume_returns_not_found_before_claiming() {
    let task_id = Uuid::from_u128(3);
    let repository = Arc::new(FakeStrategyTaskRepository::new(None, Vec::new()));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = summarize_result(use_cases.resume(&agent_client, task_id).await);
    let actual = observe(result, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        actual,
        ResumeObservation {
            result: ResumeResult::NotFound(task_id),
            requests: Vec::new(),
            task: None,
            begun_transactions: 0,
            committed_transactions: 0,
        },
    );
}

#[rstest]
#[case::failed(StrategyTaskPhase::Failed)]
#[case::completed(StrategyTaskPhase::Completed)]
#[tokio::test]
async fn resume_replays_saved_steps_and_updates_the_existing_task(
    #[case] phase: StrategyTaskPhase,
) {
    let task = task(phase, None);
    let task_id = task.task_id;
    let completed_step_id = Uuid::from_u128(5);
    let failed_step_id = Uuid::from_u128(6);
    let steps = vec![
        step(
            task_id,
            completed_step_id,
            "planning",
            StrategyTaskStepStatus::Completed,
            1,
        ),
        step(
            task_id,
            failed_step_id,
            "research",
            StrategyTaskStepStatus::Failed,
            2,
        ),
    ];
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), steps));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_next_task_id("fictional-resumed-task")
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = summarize_result(use_cases.resume(&agent_client, task_id).await);
    let actual = observe(result, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        actual,
        ResumeObservation {
            result: ResumeResult::Submitted {
                task_id,
                a2a_task_id: "fictional-resumed-task".to_string(),
            },
            requests: vec![RequestView {
                strategy_id: Uuid::from_u128(4),
                prompt: "prompt".to_string(),
                purpose: Some("fictional-purpose".to_string()),
                resume_steps: Some(vec![
                    serde_json::json!({
                        "execution_step_id": completed_step_id,
                        "phase_key": "planning",
                        "label": "planning",
                        "model": "fictional-model",
                        "status": "completed",
                        "started_at": "2026-01-01T00:00:00+00:00",
                        "trace_id": "trace-placeholder",
                        "span_id": "span-placeholder",
                    }),
                    serde_json::json!({
                        "execution_step_id": failed_step_id,
                        "phase_key": "research",
                        "label": "research",
                        "model": "fictional-model",
                        "status": "failed",
                        "started_at": "2026-01-01T00:00:00+00:00",
                        "trace_id": "trace-placeholder",
                        "span_id": "span-placeholder",
                    }),
                ]),
                as_of: Some(fixed_time()),
            }],
            task: Some(TaskView {
                phase: StrategyTaskPhase::Running,
                a2a_task_id: Some("fictional-resumed-task".to_string()),
                error_summary: None,
                result_text: None,
                auto_resumed_at_is_set: false,
                as_of: Some(fixed_time()),
            }),
            begun_transactions: 2,
            committed_transactions: 2,
        },
    );
}

#[tokio::test]
async fn resume_does_not_submit_again_after_claiming_the_task() {
    let task = task(StrategyTaskPhase::Failed, None);
    let task_id = task.task_id;
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), Vec::new()));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_next_task_id("fictional-first-resume")
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let first = summarize_result(use_cases.resume(&agent_client, task_id).await);
    let second = summarize_result(use_cases.resume(&agent_client, task_id).await);
    let actual = observe(second, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        (first, actual,),
        (
            ResumeResult::Submitted {
                task_id,
                a2a_task_id: "fictional-first-resume".to_string(),
            },
            ResumeObservation {
                result: ResumeResult::NotResumable {
                    task_id,
                    phase: "running".to_string(),
                },
                requests: vec![RequestView {
                    strategy_id: Uuid::from_u128(4),
                    prompt: "prompt".to_string(),
                    purpose: Some("fictional-purpose".to_string()),
                    resume_steps: None,
                    as_of: Some(fixed_time()),
                }],
                task: Some(TaskView {
                    phase: StrategyTaskPhase::Running,
                    a2a_task_id: Some("fictional-first-resume".to_string()),
                    error_summary: None,
                    result_text: None,
                    auto_resumed_at_is_set: false,
                    as_of: Some(fixed_time()),
                }),
                begun_transactions: 3,
                committed_transactions: 3,
            },
        ),
    );
}

#[tokio::test]
async fn auto_resume_marks_task_when_submission_succeeds() {
    let task = task(StrategyTaskPhase::Failed, None);
    let task_id = task.task_id;
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), Vec::new()));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    agent_client.set_next_task_id("fictional-auto-resume").await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = summarize_result(use_cases.auto_resume(&agent_client, task_id).await);
    let actual = observe(result, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        actual,
        ResumeObservation {
            result: ResumeResult::Submitted {
                task_id,
                a2a_task_id: "fictional-auto-resume".to_string(),
            },
            requests: vec![RequestView {
                strategy_id: Uuid::from_u128(4),
                prompt: "prompt".to_string(),
                purpose: Some("fictional-purpose".to_string()),
                resume_steps: None,
                as_of: Some(fixed_time()),
            }],
            task: Some(TaskView {
                phase: StrategyTaskPhase::Running,
                a2a_task_id: Some("fictional-auto-resume".to_string()),
                error_summary: None,
                result_text: None,
                auto_resumed_at_is_set: true,
                as_of: Some(fixed_time()),
            }),
            begun_transactions: 2,
            committed_transactions: 2,
        },
    );
}

#[tokio::test]
async fn auto_resume_rejects_a_task_that_was_already_auto_resumed() {
    let task = task(StrategyTaskPhase::Failed, None);
    let task_id = task.task_id;
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), Vec::new()));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_next_task_id("fictional-first-auto-resume")
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let first = summarize_result(use_cases.auto_resume(&agent_client, task_id).await);
    repository.set_phase(StrategyTaskPhase::Failed).await;
    let second = summarize_result(use_cases.auto_resume(&agent_client, task_id).await);
    let actual = observe(second, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        (first, actual),
        (
            ResumeResult::Submitted {
                task_id,
                a2a_task_id: "fictional-first-auto-resume".to_string(),
            },
            ResumeObservation {
                result: ResumeResult::NotResumable {
                    task_id,
                    phase: "failed".to_string(),
                },
                requests: vec![RequestView {
                    strategy_id: Uuid::from_u128(4),
                    prompt: "prompt".to_string(),
                    purpose: Some("fictional-purpose".to_string()),
                    resume_steps: None,
                    as_of: Some(fixed_time()),
                }],
                task: Some(TaskView {
                    phase: StrategyTaskPhase::Failed,
                    a2a_task_id: Some("fictional-first-auto-resume".to_string()),
                    error_summary: None,
                    result_text: None,
                    auto_resumed_at_is_set: true,
                    as_of: Some(fixed_time()),
                }),
                begun_transactions: 3,
                committed_transactions: 3,
            },
        ),
    );
}

#[tokio::test]
async fn auto_resume_keeps_claim_marker_when_submission_fails() {
    let task = task(StrategyTaskPhase::Failed, None);
    let task_id = task.task_id;
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), Vec::new()));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_submit_error(AgentTaskError::NotConfigured)
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = summarize_result(use_cases.auto_resume(&agent_client, task_id).await);
    let actual = observe(result, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        actual,
        ResumeObservation {
            result: ResumeResult::AgentTaskFailure(
                "agent task client is not configured".to_string(),
            ),
            requests: Vec::new(),
            task: Some(TaskView {
                phase: StrategyTaskPhase::Failed,
                a2a_task_id: None,
                error_summary: Some(
                    "agent task resume submission failed: agent task client is not configured"
                        .to_string(),
                ),
                result_text: Some("stale result".to_string()),
                auto_resumed_at_is_set: true,
                as_of: Some(fixed_time()),
            }),
            begun_transactions: 2,
            committed_transactions: 2,
        },
    );
}

#[tokio::test]
async fn manual_resume_preserves_an_existing_auto_resume_marker() {
    let task = task(StrategyTaskPhase::Failed, Some(fixed_time()));
    let task_id = task.task_id;
    let repository = Arc::new(FakeStrategyTaskRepository::new(Some(task), Vec::new()));
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let agent_client = FakeAgentTaskClient::new();
    agent_client
        .set_next_task_id("fictional-manual-resume")
        .await;
    let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());

    let result = summarize_result(use_cases.resume(&agent_client, task_id).await);
    let actual = observe(result, &repository, &agent_client, &unit_of_work).await;

    assert_eq!(
        actual,
        ResumeObservation {
            result: ResumeResult::Submitted {
                task_id,
                a2a_task_id: "fictional-manual-resume".to_string(),
            },
            requests: vec![RequestView {
                strategy_id: Uuid::from_u128(4),
                prompt: "prompt".to_string(),
                purpose: Some("fictional-purpose".to_string()),
                resume_steps: None,
                as_of: Some(fixed_time()),
            }],
            task: Some(TaskView {
                phase: StrategyTaskPhase::Running,
                a2a_task_id: Some("fictional-manual-resume".to_string()),
                error_summary: None,
                result_text: None,
                auto_resumed_at_is_set: true,
                as_of: Some(fixed_time()),
            }),
            begun_transactions: 2,
            committed_transactions: 2,
        },
    );
}
