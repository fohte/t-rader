use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::strategy_task::repository::{StrategyTaskRepository, StrategyTaskRepositoryError};
use crate::strategy_task::{
    StrategyTask, StrategyTaskPhase, StrategyTaskStep, StrategyTaskStepStatus, StrategyTaskUpdate,
    TaskListQuery,
};
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Default)]
pub(super) struct RepositoryState {
    pub(super) task: Option<StrategyTask>,
    pub(super) steps: Vec<StrategyTaskStep>,
}

pub(super) struct FakeStrategyTaskRepository {
    pub(super) state: Mutex<RepositoryState>,
    pub(super) update_phases: Mutex<Vec<Option<StrategyTaskPhase>>>,
    agent_config_exists: bool,
    fail_failed_update: bool,
}

impl FakeStrategyTaskRepository {
    pub(super) fn new(task: Option<StrategyTask>, steps: Vec<StrategyTaskStep>) -> Self {
        Self {
            state: Mutex::new(RepositoryState { task, steps }),
            update_phases: Mutex::new(Vec::new()),
            agent_config_exists: true,
            fail_failed_update: false,
        }
    }

    pub(super) fn without_agent_config(task: Option<StrategyTask>) -> Self {
        Self {
            agent_config_exists: false,
            ..Self::new(task, Vec::new())
        }
    }

    pub(super) fn failing_failed_update(task: Option<StrategyTask>) -> Self {
        Self {
            fail_failed_update: true,
            ..Self::new(task, Vec::new())
        }
    }

    pub(super) async fn set_phase(&self, phase: StrategyTaskPhase) {
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
        Ok(self.agent_config_exists)
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
        self.update_phases.lock().await.push(update.phase);
        if self.fail_failed_update && update.phase == Some(StrategyTaskPhase::Failed) {
            return Err(StrategyTaskRepositoryError::Database(
                PersistenceError::Database("simulated update failure".to_string()),
            ));
        }

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
