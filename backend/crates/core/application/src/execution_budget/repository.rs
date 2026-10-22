use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use uuid::Uuid;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::{
    BudgetReservationCommand, BudgetReservationResult, BudgetReservationState,
    BudgetReservationTransition, ExecutionBudgetRepositoryError,
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
    ) -> Result<BudgetReservationTransition, ExecutionBudgetRepositoryError>;

    async fn complete_call(
        &self,
        transaction: &UnitOfWorkTransaction,
        task_id: Uuid,
        call_id: &str,
        response: serde_json::Value,
        now: DateTime<FixedOffset>,
    ) -> Result<(), ExecutionBudgetRepositoryError>;
}

pub type SharedExecutionBudgetRepository = Arc<dyn ExecutionBudgetRepository + Send + Sync>;
