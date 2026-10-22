use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    execution_budget::{
        BudgetReservation, BudgetReservationCommand, BudgetReservationResult,
        BudgetReservationState, ExecutionBudgetRepository, ExecutionBudgetRepositoryError,
    },
    unit_of_work::UnitOfWorkTransaction,
};

#[derive(Default)]
pub(crate) struct FakeExecutionBudgetRepository {
    deadlines: Mutex<HashMap<Uuid, DateTime<FixedOffset>>>,
}

impl FakeExecutionBudgetRepository {
    pub(crate) async fn initialized_deadline(
        &self,
        task_id: Uuid,
    ) -> Option<DateTime<FixedOffset>> {
        self.deadlines.lock().await.get(&task_id).cloned()
    }
}

#[async_trait]
impl ExecutionBudgetRepository for FakeExecutionBudgetRepository {
    async fn initialize_task(
        &self,
        _transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        deadline_at: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError> {
        let mut deadlines = self.deadlines.lock().await;
        if deadlines
            .get(&task_id)
            .is_some_and(|stored| stored.timestamp_micros() != deadline_at.timestamp_micros())
        {
            return Err(ExecutionBudgetRepositoryError::ReservationConflict);
        }
        deadlines.insert(task_id, deadline_at);
        Ok(())
    }

    async fn reserve(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _command: BudgetReservationCommand,
    ) -> Result<BudgetReservationResult, ExecutionBudgetRepositoryError> {
        Err(ExecutionBudgetRepositoryError::StateNotFound)
    }

    async fn transition_call(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _task_id: Uuid,
        _call_id: &str,
        _state: BudgetReservationState,
        _now: DateTime<FixedOffset>,
    ) -> Result<Vec<BudgetReservation>, ExecutionBudgetRepositoryError> {
        Err(ExecutionBudgetRepositoryError::StateNotFound)
    }
}
