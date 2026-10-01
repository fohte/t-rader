use std::sync::Arc;

use chrono::{DateTime, FixedOffset};
use rstest::rstest;
use uuid::Uuid;

use crate::agent_task_client::{AgentTaskError, FakeAgentTaskClient, SubmitAgentTask};
use crate::strategy_task::test_support::FakeStrategyTaskRepository;
use crate::strategy_task::{
    ResumeTaskError, StrategyTask, StrategyTaskPhase, StrategyTaskStep, StrategyTaskStepStatus,
    StrategyTaskUseCases,
};
use crate::unit_of_work::FakeUnitOfWork;

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

struct ResumeHarness {
    repository: Arc<FakeStrategyTaskRepository>,
    unit_of_work: Arc<FakeUnitOfWork>,
    agent_client: FakeAgentTaskClient,
    use_cases: StrategyTaskUseCases,
}

impl ResumeHarness {
    fn new(task: Option<StrategyTask>, steps: Vec<StrategyTaskStep>) -> Self {
        let repository = Arc::new(FakeStrategyTaskRepository::new(task, steps));
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let agent_client = FakeAgentTaskClient::new();
        let use_cases = StrategyTaskUseCases::new(unit_of_work.clone(), repository.clone());
        Self {
            repository,
            unit_of_work,
            agent_client,
            use_cases,
        }
    }

    async fn observe(&self, result: ResumeResult) -> ResumeObservation {
        let requests = self
            .agent_client
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
        let task = self
            .repository
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
            begun_transactions: self.unit_of_work.begun.lock().await.len(),
            committed_transactions: self.unit_of_work.committed.lock().await.len(),
        }
    }
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
    let harness = ResumeHarness::new(Some(task), steps);

    let result = summarize_result(
        harness
            .use_cases
            .resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(result).await;

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
    let harness = ResumeHarness::new(None, Vec::new());

    let result = summarize_result(
        harness
            .use_cases
            .resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(result).await;

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
    let harness = ResumeHarness::new(Some(task), steps);
    harness
        .agent_client
        .set_next_task_id("fictional-resumed-task")
        .await;

    let result = summarize_result(
        harness
            .use_cases
            .resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(result).await;

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
    let harness = ResumeHarness::new(Some(task), Vec::new());
    harness
        .agent_client
        .set_next_task_id("fictional-first-resume")
        .await;

    let first = summarize_result(
        harness
            .use_cases
            .resume(&harness.agent_client, task_id)
            .await,
    );
    let second = summarize_result(
        harness
            .use_cases
            .resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(second).await;

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
    let harness = ResumeHarness::new(Some(task), Vec::new());
    harness
        .agent_client
        .set_next_task_id("fictional-auto-resume")
        .await;

    let result = summarize_result(
        harness
            .use_cases
            .auto_resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(result).await;

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
    let harness = ResumeHarness::new(Some(task), Vec::new());
    harness
        .agent_client
        .set_next_task_id("fictional-first-auto-resume")
        .await;

    let first = summarize_result(
        harness
            .use_cases
            .auto_resume(&harness.agent_client, task_id)
            .await,
    );
    harness
        .repository
        .set_phase(StrategyTaskPhase::Failed)
        .await;
    let second = summarize_result(
        harness
            .use_cases
            .auto_resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(second).await;

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
    let harness = ResumeHarness::new(Some(task), Vec::new());
    harness
        .agent_client
        .set_submit_error(AgentTaskError::NotConfigured)
        .await;

    let result = summarize_result(
        harness
            .use_cases
            .auto_resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(result).await;

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
    let harness = ResumeHarness::new(Some(task), Vec::new());
    harness
        .agent_client
        .set_next_task_id("fictional-manual-resume")
        .await;

    let result = summarize_result(
        harness
            .use_cases
            .resume(&harness.agent_client, task_id)
            .await,
    );
    let actual = harness.observe(result).await;

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
