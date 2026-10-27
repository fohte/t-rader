use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::Value;

use crate::unit_of_work::UnitOfWorkTransaction;

use super::{TavilyCreditBudgetConfig, TavilyCreditBudgetDenial, TavilyCreditBudgetOperation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetReadiness {
    Ready,
    NotInitialized,
    AccountMismatch,
    ConfigurationChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExistingReservation {
    Missing,
    Prepared,
    InProgress,
    Replay(Value),
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationOutcome {
    Reserved {
        daily_used: Decimal,
        monthly_used: Decimal,
    },
    ReusedPrepared {
        daily_used: Decimal,
        monthly_used: Decimal,
    },
    Replay(Value),
    InProgress,
    Denied {
        denial: TavilyCreditBudgetDenial,
        daily_used: Decimal,
        monthly_used: Decimal,
    },
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReservationClaim {
    Claimed,
    InProgress,
    Replay(Value),
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapCommand {
    pub config: TavilyCreditBudgetConfig,
    pub monthly_provider_usage: Decimal,
    pub month_start: NaiveDate,
    pub day_start: NaiveDate,
    pub daily_blocked_until: DateTime<Utc>,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservationCommand {
    pub config: TavilyCreditBudgetConfig,
    pub call_id: String,
    pub request_fingerprint: String,
    pub operation: TavilyCreditBudgetOperation,
    pub monthly_provider_usage: Decimal,
    pub month_start: NaiveDate,
    pub day_start: NaiveDate,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDenialCommand {
    pub call_id: String,
    pub request_fingerprint: String,
    pub operation: TavilyCreditBudgetOperation,
    pub outcome: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCacheHitCommand {
    pub call_id: String,
    pub request_fingerprint: String,
    pub operation: TavilyCreditBudgetOperation,
    pub outcome: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationFailureState {
    Unknown,
    ProviderRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishFailureCommand {
    pub call_id: String,
    pub request_fingerprint: String,
    pub state: ReservationFailureState,
    pub outcome: Value,
    pub now: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum TavilyCreditBudgetRepositoryError {
    #[error("database operation failed")]
    Database,
    #[error("reservation state is inconsistent")]
    InvalidState,
}

#[async_trait]
pub trait TavilyCreditBudgetRepository: Send + Sync {
    async fn readiness(
        &self,
        transaction: &UnitOfWorkTransaction,
        config: &TavilyCreditBudgetConfig,
    ) -> Result<BudgetReadiness, TavilyCreditBudgetRepositoryError>;

    async fn initialize(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: BootstrapCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError>;

    async fn find_reservation(
        &self,
        transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
    ) -> Result<ExistingReservation, TavilyCreditBudgetRepositoryError>;

    async fn reserve(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: ReservationCommand,
    ) -> Result<ReservationOutcome, TavilyCreditBudgetRepositoryError>;

    async fn record_denial(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: RecordDenialCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError>;

    async fn record_cache_hit(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: RecordCacheHitCommand,
    ) -> Result<ReservationClaim, TavilyCreditBudgetRepositoryError>;

    async fn claim(
        &self,
        transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
        now: DateTime<Utc>,
    ) -> Result<ReservationClaim, TavilyCreditBudgetRepositoryError>;

    async fn settle(
        &self,
        transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
        actual_credits: Decimal,
        outcome: Value,
        now: DateTime<Utc>,
    ) -> Result<(), TavilyCreditBudgetRepositoryError>;

    async fn finish_failure(
        &self,
        transaction: &UnitOfWorkTransaction,
        command: FinishFailureCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError>;
}

pub type SharedTavilyCreditBudgetRepository = Arc<dyn TavilyCreditBudgetRepository + Send + Sync>;
