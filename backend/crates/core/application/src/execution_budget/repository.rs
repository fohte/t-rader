use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::{
    BudgetReservation, BudgetReservationCommand, BudgetReservationResult, BudgetReservationState,
    ExecutionBudgetRepositoryError,
};

#[async_trait]
pub trait ExecutionBudgetRepository: Send + Sync {
    async fn initialize_task(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        deadline_at: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError>;

    async fn reserve(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: BudgetReservationCommand,
    ) -> Result<BudgetReservationResult, ExecutionBudgetRepositoryError>;

    async fn transition_call(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        call_id: &str,
        state: BudgetReservationState,
        now: DateTime<FixedOffset>,
    ) -> Result<Vec<BudgetReservation>, ExecutionBudgetRepositoryError>;

    async fn find_deadline(
        &self,
        task_id: Uuid,
    ) -> Result<Option<DateTime<FixedOffset>>, ExecutionBudgetRepositoryError>;
}

pub type SharedExecutionBudgetRepository = Arc<dyn ExecutionBudgetRepository + Send + Sync>;
