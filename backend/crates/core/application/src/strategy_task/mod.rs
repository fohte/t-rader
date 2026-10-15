mod queries;
mod reconcile;
mod resume;
mod submit;

mod error;
mod reconcile_job_queue;
mod reconcile_job_use_cases;
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
pub use reconcile_job_queue::{
    STRATEGY_TASK_RECONCILE_JOB_IDENTIFIER, STRATEGY_TASK_RECONCILE_QUEUE_NAME,
    SharedStrategyTaskReconcileJobQueue, StrategyTaskReconcileJobQueue,
    StrategyTaskReconcileJobQueueError,
};
pub use reconcile_job_use_cases::StrategyTaskReconcileJobUseCases;
pub use repository::{
    SharedStrategyTaskRepository, StrategyTaskRepository, StrategyTaskRepositoryError,
};
pub use types::{
    StrategyTask, StrategyTaskContext, StrategyTaskPhase, StrategyTaskStep, StrategyTaskStepStatus,
    StrategyTaskUpdate, SubmittedTask, TaskListQuery, TaskSource, TaskStatusView,
};

use crate::agent_config::graph::parse_agent_graph;
use crate::unit_of_work::SharedUnitOfWork;

pub const DEADLINE_DURATION: chrono::Duration = chrono::Duration::minutes(15);
pub const DEFAULT_PURPOSE: &str = "default";

pub(super) fn deadline_duration_from_agent_graph(
    agent_graph: &str,
) -> Result<chrono::Duration, crate::agent_config::graph::AgentGraphError> {
    let timeout_minutes = parse_agent_graph(agent_graph)?
        .and_then(|config| config.timeout_minutes)
        .map(i64::from);
    Ok(timeout_minutes.map_or(DEADLINE_DURATION, chrono::Duration::minutes))
}

#[derive(Clone)]
pub struct StrategyTaskUseCases {
    pub(super) unit_of_work: SharedUnitOfWork,
    pub(super) repository: SharedStrategyTaskRepository,
}

impl StrategyTaskUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedStrategyTaskRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }
}
