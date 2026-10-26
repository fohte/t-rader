use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use core_application::tavily_credit_budget::{
    BootstrapCommand, BudgetReadiness, ExistingReservation, FinishFailureCommand,
    RecordDenialCommand, ReservationClaim, ReservationCommand, ReservationFailureState,
    ReservationOutcome, SharedTavilyCreditBudgetRepository, SharedTavilyCreditBudgetUseCases,
    SystemTavilyCreditBudgetClock, TavilyCreditBudgetConfig, TavilyCreditBudgetDenial,
    TavilyCreditBudgetOperation, TavilyCreditBudgetReason, TavilyCreditBudgetRepository,
    TavilyCreditBudgetRepositoryError, TavilyCreditBudgetScope, TavilyCreditBudgetUseCases,
};
use core_application::unit_of_work::{FakeUnitOfWork, UnitOfWorkTransaction};
use core_application::web_search::SharedWebSearchClient;
use rmcp::ErrorData as McpError;
use rmcp::model::ErrorCode;
use rust_decimal::Decimal;
use serde_json::Value;
use tokio::sync::Mutex;

#[derive(Clone)]
struct TestCreditBudgetRepository(Arc<Mutex<TestState>>);

struct TestState {
    config: TavilyCreditBudgetConfig,
    daily_used: Decimal,
    monthly_used: Decimal,
    reservations: std::collections::HashMap<String, TestReservation>,
}

#[derive(Clone)]
struct TestReservation {
    fingerprint: String,
    operation: TavilyCreditBudgetOperation,
    state: TestReservationState,
    reserved: Decimal,
    settled: Option<Decimal>,
    outcome: Option<Value>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TestReservationState {
    Prepared,
    Dispatching,
    Settled,
    Unknown,
    ProviderRejected,
    Denied,
}

impl Default for TestCreditBudgetRepository {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(TestState {
            config: TavilyCreditBudgetConfig::free_tier("example-account".into(), true)
                .expect("valid test account configuration"),
            daily_used: Decimal::ZERO,
            monthly_used: Decimal::ZERO,
            reservations: std::collections::HashMap::new(),
        })))
    }
}

pub(super) fn test_tavily_credit_budget(
    provider: Option<SharedWebSearchClient>,
) -> SharedTavilyCreditBudgetUseCases {
    let config = TavilyCreditBudgetConfig::free_tier("example-account".into(), true);
    let repository: SharedTavilyCreditBudgetRepository =
        Arc::new(TestCreditBudgetRepository::default());
    Arc::new(TavilyCreditBudgetUseCases::new(
        Arc::new(FakeUnitOfWork::new()),
        repository,
        provider,
        config,
        Arc::new(SystemTavilyCreditBudgetClock),
    ))
}

pub(super) fn expected_denial(
    reason: TavilyCreditBudgetReason,
    scope: TavilyCreditBudgetScope,
    provider_status: Option<u16>,
) -> McpError {
    let mut denial = TavilyCreditBudgetDenial::new(reason, scope, None);
    if let Some(status) = provider_status {
        denial = denial.with_provider_status(status);
    }
    McpError::new(
        ErrorCode(-32050),
        denial.instruction(),
        Some(serde_json::json!({
            "reason": denial.reason.as_str(),
            "scope": denial.scope.as_str(),
            "next_reset": denial.next_reset,
            "provider_status": denial.provider_status,
        })),
    )
}

#[async_trait]
impl TavilyCreditBudgetRepository for TestCreditBudgetRepository {
    async fn readiness(
        &self,
        _transaction: &UnitOfWorkTransaction,
        config: &TavilyCreditBudgetConfig,
    ) -> Result<BudgetReadiness, TavilyCreditBudgetRepositoryError> {
        Ok(if self.0.lock().await.config == *config {
            BudgetReadiness::Ready
        } else {
            BudgetReadiness::ConfigurationChanged
        })
    }

    async fn initialize(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: BootstrapCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        state.config = command.config;
        state.daily_used = Decimal::ZERO;
        state.monthly_used = command.monthly_provider_usage;
        Ok(())
    }

    async fn find_reservation(
        &self,
        _transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
    ) -> Result<ExistingReservation, TavilyCreditBudgetRepositoryError> {
        let state = self.0.lock().await;
        let Some(reservation) = state.reservations.get(call_id) else {
            return Ok(ExistingReservation::Missing);
        };
        if reservation.fingerprint != request_fingerprint {
            return Ok(ExistingReservation::Conflict);
        }
        Ok(match reservation.state {
            TestReservationState::Prepared => ExistingReservation::Prepared,
            TestReservationState::Dispatching => ExistingReservation::InProgress,
            _ => ExistingReservation::Replay(
                reservation
                    .outcome
                    .clone()
                    .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?,
            ),
        })
    }

    async fn reserve(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: ReservationCommand,
    ) -> Result<ReservationOutcome, TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        if state.config != command.config {
            return Ok(ReservationOutcome::Denied {
                denial: TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::ConfigurationChanged,
                    TavilyCreditBudgetScope::Account,
                    None,
                ),
                daily_used: state.daily_used,
                monthly_used: state.monthly_used,
            });
        }
        if let Some(reservation) = state.reservations.get(&command.call_id) {
            if reservation.fingerprint != command.request_fingerprint
                || reservation.operation != command.operation
            {
                return Ok(ReservationOutcome::Conflict);
            }
            return Ok(match reservation.state {
                TestReservationState::Prepared => ReservationOutcome::ReusedPrepared {
                    daily_used: state.daily_used,
                    monthly_used: state.monthly_used,
                },
                TestReservationState::Dispatching => ReservationOutcome::InProgress,
                _ => ReservationOutcome::Replay(
                    reservation
                        .outcome
                        .clone()
                        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?,
                ),
            });
        }
        let reserved = command.operation.reservation();
        if state.daily_used + reserved > command.config.daily_limit() {
            return Ok(ReservationOutcome::Denied {
                denial: TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    None,
                ),
                daily_used: state.daily_used,
                monthly_used: state.monthly_used,
            });
        }
        if state.monthly_used + reserved > command.config.monthly_limit() {
            return Ok(ReservationOutcome::Denied {
                denial: TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::MonthlyLimitExceeded,
                    TavilyCreditBudgetScope::Monthly,
                    None,
                ),
                daily_used: state.daily_used,
                monthly_used: state.monthly_used,
            });
        }
        state.daily_used += reserved;
        state.monthly_used += reserved;
        state.reservations.insert(
            command.call_id,
            TestReservation {
                fingerprint: command.request_fingerprint,
                operation: command.operation,
                state: TestReservationState::Prepared,
                reserved,
                settled: None,
                outcome: None,
            },
        );
        Ok(ReservationOutcome::Reserved {
            daily_used: state.daily_used,
            monthly_used: state.monthly_used,
        })
    }

    async fn record_denial(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: RecordDenialCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        self.0.lock().await.reservations.insert(
            command.call_id.clone(),
            TestReservation {
                fingerprint: command.request_fingerprint,
                operation: command.operation,
                state: TestReservationState::Denied,
                reserved: Decimal::ZERO,
                settled: Some(Decimal::ZERO),
                outcome: Some(command.outcome),
            },
        );
        Ok(())
    }

    async fn claim(
        &self,
        _transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
        _now: DateTime<Utc>,
    ) -> Result<ReservationClaim, TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        let reservation = state
            .reservations
            .get_mut(call_id)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        if reservation.fingerprint != request_fingerprint {
            return Ok(ReservationClaim::Conflict);
        }
        match reservation.state {
            TestReservationState::Prepared => {
                reservation.state = TestReservationState::Dispatching;
                Ok(ReservationClaim::Claimed)
            }
            TestReservationState::Dispatching => Ok(ReservationClaim::InProgress),
            _ => Ok(ReservationClaim::Replay(
                reservation
                    .outcome
                    .clone()
                    .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?,
            )),
        }
    }

    async fn settle(
        &self,
        _transaction: &UnitOfWorkTransaction,
        call_id: &str,
        request_fingerprint: &str,
        actual_credits: Decimal,
        outcome: Value,
        _now: DateTime<Utc>,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        let reservation = state
            .reservations
            .get_mut(call_id)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        if reservation.fingerprint != request_fingerprint
            || reservation.state != TestReservationState::Dispatching
        {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        let adjustment = actual_credits - reservation.reserved;
        state.daily_used += adjustment;
        state.monthly_used += adjustment;
        let reservation = state
            .reservations
            .get_mut(call_id)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        reservation.state = TestReservationState::Settled;
        reservation.settled = Some(actual_credits);
        reservation.outcome = Some(outcome);
        Ok(())
    }

    async fn finish_failure(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: FinishFailureCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        let reservation = state
            .reservations
            .get_mut(&command.call_id)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        if reservation.fingerprint != command.request_fingerprint
            || reservation.state != TestReservationState::Dispatching
        {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        reservation.state = match command.state {
            ReservationFailureState::Unknown => TestReservationState::Unknown,
            ReservationFailureState::ProviderRejected => TestReservationState::ProviderRejected,
        };
        reservation.outcome = Some(command.outcome);
        Ok(())
    }
}
