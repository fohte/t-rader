mod completion_event;
mod queries;
mod reconcile;
mod resume;
mod submit;

mod error;
mod reconcile_job;
mod reconcile_lock;
mod reconcile_lock_use_cases;
mod reconcile_use_cases;
mod repository;
mod types;

#[cfg(all(test, feature = "test-support"))]
mod resume_tests;
#[cfg(all(test, feature = "test-support"))]
mod test_support;
#[cfg(all(test, feature = "test-support"))]
mod tests;

pub use error::{
    GetTaskError, ListTasksError, ReconcileTaskError, ResumeTaskError, SubmitTaskError,
};
pub use reconcile_job::{
    STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER, STRATEGY_TASK_RECONCILE_QUEUE_NAME,
};
pub use reconcile_lock::{
    SharedStrategyTaskReconcileLock, StrategyTaskReconcileLock, StrategyTaskReconcileLockGuard,
};
pub use reconcile_lock_use_cases::StrategyTaskReconcileLockUseCases;
pub use reconcile_use_cases::StrategyTaskReconcileUseCases;
pub use repository::{
    SharedStrategyTaskRepository, StrategyTaskApplyResult, StrategyTaskRepository,
    StrategyTaskRepositoryError,
};
pub(crate) use types::PreparedStrategyTaskSubmission;
pub use types::{
    StrategyTask, StrategyTaskContext, StrategyTaskOrigin, StrategyTaskPhase, StrategyTaskStep,
    StrategyTaskStepStatus, StrategyTaskUpdate, SubmittedTask, TaskListQuery, TaskSource,
    TaskStatusView,
};

use crate::agent_config::graph::parse_agent_graph;
use crate::event::SharedEventRepository;
use crate::event_dispatch_job::EventDispatchJobUseCases;
use crate::unit_of_work::SharedUnitOfWork;

const DEFAULT_DEADLINE_DURATION: chrono::Duration = chrono::Duration::minutes(15);
pub const DEFAULT_PURPOSE: &str = "default";

pub(super) fn deadline_duration_from_agent_graph(agent_graph: &str) -> chrono::Duration {
    let timeout_minutes = match parse_agent_graph(agent_graph) {
        Ok(config) => config
            .and_then(|config| config.timeout_minutes)
            .map(i64::from),
        Err(_) => {
            tracing::warn!("stored agent_graph could not be parsed; using default task deadline");
            None
        }
    };
    timeout_minutes.map_or(DEFAULT_DEADLINE_DURATION, chrono::Duration::minutes)
}

#[derive(Clone)]
pub struct StrategyTaskUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedStrategyTaskRepository,
    completion_events: Option<CompletionEventDependencies>,
}

#[derive(Clone)]
struct CompletionEventDependencies {
    repository: SharedEventRepository,
    dispatch_job: EventDispatchJobUseCases,
}

impl StrategyTaskUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedStrategyTaskRepository) -> Self {
        Self {
            unit_of_work,
            repository,
            completion_events: None,
        }
    }

    pub fn with_completion_events(
        mut self,
        repository: SharedEventRepository,
        dispatch_job: EventDispatchJobUseCases,
    ) -> Self {
        self.completion_events = Some(CompletionEventDependencies {
            repository,
            dispatch_job,
        });
        self
    }
}
