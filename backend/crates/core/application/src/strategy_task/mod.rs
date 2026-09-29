mod queries;
mod reconcile;
mod resume;
mod submit;

mod error;
mod repository;
mod types;

pub use error::{
    GetTaskError, ListTasksError, ReconcileTaskError, ResumeTaskError, SubmitTaskError,
};
pub use repository::{
    SharedStrategyTaskRepository, StrategyTaskRepository, StrategyTaskRepositoryError,
};
pub use types::{
    StrategyTask, StrategyTaskPhase, StrategyTaskStep, StrategyTaskStepStatus, StrategyTaskUpdate,
    SubmittedTask, TaskListQuery, TaskSource, TaskStatusView,
};

use crate::unit_of_work::SharedUnitOfWork;

pub const DEADLINE_DURATION: chrono::Duration = chrono::Duration::minutes(15);
pub const DEFAULT_PURPOSE: &str = "default";

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

pub fn phase_str(phase: StrategyTaskPhase) -> &'static str {
    phase.as_str()
}
