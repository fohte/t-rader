use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use rstest::rstest;
use rust_decimal::Decimal;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    tavily_credit_budget::{
        BootstrapCommand, BudgetReadiness, ExistingReservation, FinishFailureCommand,
        RecordDenialCommand, ReservationClaim, ReservationCommand, ReservationFailureState,
        ReservationOutcome, TavilyCreditBudgetClock, TavilyCreditBudgetConfig,
        TavilyCreditBudgetDenial, TavilyCreditBudgetOperation, TavilyCreditBudgetReason,
        TavilyCreditBudgetRepository, TavilyCreditBudgetRepositoryError, TavilyCreditBudgetScope,
        TavilyCreditBudgetUseCases, TavilyCreditBudgetUseCasesError,
        TavilyCreditBudgetUseCasesPort,
    },
    tavily_retrieval_cache::{
        SharedTavilyRetrievalCacheRepository, TavilyCredentialGeneration,
        TavilyRetrievalCacheEntry, TavilyRetrievalCacheError, TavilyRetrievalCacheKey,
        TavilyRetrievalCacheLease, TavilyRetrievalCacheLookup, TavilyRetrievalCacheRepository,
        TavilyRetrievalCacheWrite,
    },
    unit_of_work::{UnitOfWork, UnitOfWorkError, UnitOfWorkTransaction},
    web_search::{
        WebPageExtractionFailure, WebPageExtractionResponse, WebSearchAccountUsage,
        WebSearchClient, WebSearchError, WebSearchResponse, WebSearchResult, WebSearchTimeRange,
        WebSearchTopic,
    },
};

#[derive(Default)]
struct TestUnitOfWork;

#[async_trait]
impl UnitOfWork for TestUnitOfWork {
    async fn begin(&self) -> Result<UnitOfWorkTransaction, UnitOfWorkError> {
        Ok(UnitOfWorkTransaction::new(()))
    }

    async fn commit(&self, transaction: UnitOfWorkTransaction) -> Result<(), UnitOfWorkError> {
        transaction
            .downcast::<()>()
            .map(|_| ())
            .map_err(|_| UnitOfWorkError::InvalidTransaction)
    }
}

struct TestRetrievalCache;

struct TestRetrievalCacheLease;

#[derive(Default)]
struct MemoryTestRetrievalCache(Arc<std::sync::Mutex<MemoryTestRetrievalCacheState>>);

#[derive(Default)]
struct MemoryTestRetrievalCacheState {
    entries: HashMap<TavilyRetrievalCacheKey, MemoryCacheEntry>,
    leased_keys: HashSet<TavilyRetrievalCacheKey>,
}

#[derive(Clone)]
struct MemoryCacheEntry {
    payload: Value,
    fetched_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

struct MemoryTestRetrievalCacheLease {
    cache: Arc<std::sync::Mutex<MemoryTestRetrievalCacheState>>,
    key: TavilyRetrievalCacheKey,
}

#[async_trait]
impl TavilyRetrievalCacheRepository for TestRetrievalCache {
    async fn acquire(
        &self,
        _key: TavilyRetrievalCacheKey,
        _now: DateTime<Utc>,
    ) -> Result<TavilyRetrievalCacheLookup, TavilyRetrievalCacheError> {
        Ok(TavilyRetrievalCacheLookup::Miss(Box::new(
            TestRetrievalCacheLease,
        )))
    }
}

#[async_trait]
impl TavilyRetrievalCacheLease for TestRetrievalCacheLease {
    async fn store(
        self: Box<Self>,
        _entry: TavilyRetrievalCacheWrite,
    ) -> Result<(), TavilyRetrievalCacheError> {
        Ok(())
    }

    async fn release(self: Box<Self>) -> Result<(), TavilyRetrievalCacheError> {
        Ok(())
    }
}

#[async_trait]
impl TavilyRetrievalCacheRepository for MemoryTestRetrievalCache {
    async fn acquire(
        &self,
        key: TavilyRetrievalCacheKey,
        now: DateTime<Utc>,
    ) -> Result<TavilyRetrievalCacheLookup, TavilyRetrievalCacheError> {
        let mut cache = self.0.lock().expect("lock memory cache");
        if let Some(entry) = cache
            .entries
            .get(&key)
            .filter(|entry| entry.expires_at > now)
        {
            return Ok(TavilyRetrievalCacheLookup::Hit(TavilyRetrievalCacheEntry {
                payload: entry.payload.clone(),
                fetched_at: entry.fetched_at,
            }));
        }
        cache.entries.remove(&key);
        if !cache.leased_keys.insert(key.clone()) {
            return Err(TavilyRetrievalCacheError::Storage);
        }
        drop(cache);
        Ok(TavilyRetrievalCacheLookup::Miss(Box::new(
            MemoryTestRetrievalCacheLease {
                cache: self.0.clone(),
                key,
            },
        )))
    }
}

#[async_trait]
impl TavilyRetrievalCacheLease for MemoryTestRetrievalCacheLease {
    async fn store(
        self: Box<Self>,
        entry: TavilyRetrievalCacheWrite,
    ) -> Result<(), TavilyRetrievalCacheError> {
        let mut cache = self.cache.lock().expect("lock memory cache");
        cache.entries.insert(
            self.key.clone(),
            MemoryCacheEntry {
                payload: entry.payload,
                fetched_at: entry.fetched_at,
                expires_at: entry.expires_at,
            },
        );
        cache.leased_keys.remove(&self.key);
        Ok(())
    }

    async fn release(self: Box<Self>) -> Result<(), TavilyRetrievalCacheError> {
        self.cache
            .lock()
            .expect("lock memory cache")
            .leased_keys
            .remove(&self.key);
        Ok(())
    }
}

#[derive(Clone)]
struct TestClock(Arc<std::sync::Mutex<DateTime<Utc>>>);

impl TestClock {
    fn at(value: DateTime<Utc>) -> Self {
        Self(Arc::new(std::sync::Mutex::new(value)))
    }

    fn set(&self, value: DateTime<Utc>) {
        *self.0.lock().expect("set test clock") = value;
    }
}

impl TavilyCreditBudgetClock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().expect("read test clock")
    }
}

#[derive(Clone)]
struct TestProvider {
    usage: Arc<std::sync::Mutex<Result<WebSearchAccountUsage, WebSearchError>>>,
    search_response: Arc<std::sync::Mutex<Result<WebSearchResponse, WebSearchError>>>,
    extract_response: Result<WebPageExtractionResponse, WebSearchError>,
    usage_calls: Arc<std::sync::atomic::AtomicUsize>,
    search_calls: Arc<std::sync::atomic::AtomicUsize>,
    extract_calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl TestProvider {
    fn new(usage: WebSearchAccountUsage) -> Self {
        Self {
            usage: Arc::new(std::sync::Mutex::new(Ok(usage))),
            search_response: Arc::new(std::sync::Mutex::new(Ok(WebSearchResponse {
                results: vec![WebSearchResult {
                    title: "Example result".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: None,
                    snippet: "Example excerpt".into(),
                }],
                credits_used: Some(Decimal::ONE),
            }))),
            extract_response: Ok(WebPageExtractionResponse {
                raw_content: Some("Example page content".into()),
                failure: None,
                credits_used: Some(Decimal::ONE),
            }),
            usage_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            search_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            extract_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    fn set_usage(&self, usage: Result<WebSearchAccountUsage, WebSearchError>) {
        *self.usage.lock().expect("set test account usage") = usage;
    }

    fn with_search_response(self, response: Result<WebSearchResponse, WebSearchError>) -> Self {
        *self.search_response.lock().expect("set search response") = response;
        self
    }

    fn set_search_response(&self, response: Result<WebSearchResponse, WebSearchError>) {
        *self.search_response.lock().expect("set search response") = response;
    }

    fn with_extract_response(
        mut self,
        response: Result<WebPageExtractionResponse, WebSearchError>,
    ) -> Self {
        self.extract_response = response;
        self
    }

    fn call_counts(&self) -> (usize, usize, usize) {
        (
            self.usage_calls.load(std::sync::atomic::Ordering::SeqCst),
            self.search_calls.load(std::sync::atomic::Ordering::SeqCst),
            self.extract_calls.load(std::sync::atomic::Ordering::SeqCst),
        )
    }
}

#[async_trait]
impl WebSearchClient for TestProvider {
    async fn search(
        &self,
        _query: &str,
        _topic: Option<WebSearchTopic>,
        _time_range: Option<WebSearchTimeRange>,
    ) -> Result<WebSearchResponse, WebSearchError> {
        self.search_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.search_response
            .lock()
            .expect("read search response")
            .clone()
    }

    async fn extract_page(&self, _url: &str) -> Result<WebPageExtractionResponse, WebSearchError> {
        self.extract_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.extract_response.clone()
    }

    async fn account_usage(&self) -> Result<WebSearchAccountUsage, WebSearchError> {
        self.usage_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.usage.lock().expect("read test account usage").clone()
    }
}

#[derive(Clone)]
struct TestRepository(Arc<Mutex<TestState>>);

#[derive(Default)]
struct TestState {
    config: Option<TavilyCreditBudgetConfig>,
    day_start: Option<NaiveDate>,
    month_start: Option<NaiveDate>,
    daily_used: Decimal,
    daily_carry: Decimal,
    monthly_used: Decimal,
    monthly_carry: Decimal,
    day_provider_usage: Decimal,
    provider_usage: Decimal,
    daily_blocked_until: Option<DateTime<Utc>>,
    reservations: HashMap<String, TestReservation>,
}

#[derive(Clone)]
struct TestReservation {
    fingerprint: String,
    operation: TavilyCreditBudgetOperation,
    state: &'static str,
    reserved: Decimal,
    settled: Option<Decimal>,
    day_start: NaiveDate,
    month_start: NaiveDate,
    outcome: Option<Value>,
    updated_at: DateTime<Utc>,
}

impl Default for TestRepository {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(TestState::default())))
    }
}

impl TestRepository {
    async fn seed_usage(&self, now: DateTime<Utc>, daily_used: Decimal, monthly_used: Decimal) {
        let mut state = self.0.lock().await;
        state.day_start = Some(now.date_naive());
        state.month_start = Some(first_of_month(now.date_naive()));
        state.day_provider_usage = state.provider_usage;
        state.daily_used = daily_used;
        state.daily_carry = Decimal::ZERO;
        state.monthly_used = monthly_used;
        state.monthly_carry = Decimal::ZERO;
    }
}

#[async_trait]
impl TavilyCreditBudgetRepository for TestRepository {
    async fn readiness(
        &self,
        _transaction: &UnitOfWorkTransaction,
        config: &TavilyCreditBudgetConfig,
    ) -> Result<BudgetReadiness, TavilyCreditBudgetRepositoryError> {
        Ok(match self.0.lock().await.config.as_ref() {
            None => BudgetReadiness::NotInitialized,
            Some(existing) if existing.account_id() != config.account_id() => {
                BudgetReadiness::AccountMismatch
            }
            Some(existing) if existing != config => BudgetReadiness::ConfigurationChanged,
            Some(_) => BudgetReadiness::Ready,
        })
    }

    async fn initialize(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: BootstrapCommand,
    ) -> Result<(), TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        if state.config.is_some() {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        state.config = Some(command.config);
        state.day_start = Some(command.day_start);
        state.month_start = Some(command.month_start);
        state.daily_used = Decimal::ZERO;
        state.daily_carry = Decimal::ZERO;
        state.monthly_used = command.monthly_provider_usage;
        state.monthly_carry = Decimal::ZERO;
        state.provider_usage = command.monthly_provider_usage;
        state.day_provider_usage = command.monthly_provider_usage;
        state.daily_blocked_until = Some(command.daily_blocked_until);
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
        reservation_existing(reservation)
    }

    async fn reserve(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: ReservationCommand,
    ) -> Result<ReservationOutcome, TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        if state.config.as_ref() != Some(&command.config) {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        if let Some(existing) = state.reservations.get(&command.call_id).cloned() {
            if existing.fingerprint != command.request_fingerprint
                || existing.operation != command.operation
            {
                return Ok(ReservationOutcome::Conflict);
            }
            if existing.state == "prepared" {
                if let Some(denial) = reconcile(&mut state, &command) {
                    return Ok(denied_outcome(denial, &state));
                }
                if state.daily_used > command.config.daily_limit() {
                    return Ok(denied_outcome(
                        denial(
                            TavilyCreditBudgetReason::DailyLimitExceeded,
                            TavilyCreditBudgetScope::Daily,
                            next_day_start(command.day_start),
                        ),
                        &state,
                    ));
                }
                if state.monthly_used > command.config.monthly_limit() {
                    return Ok(denied_outcome(
                        denial(
                            TavilyCreditBudgetReason::MonthlyLimitExceeded,
                            TavilyCreditBudgetScope::Monthly,
                            next_month_start(command.month_start),
                        ),
                        &state,
                    ));
                }
                if existing.day_start < command.day_start {
                    state.daily_carry -= existing.reserved;
                }
                if existing.month_start < command.month_start {
                    state.monthly_carry -= existing.reserved;
                }
                let reservation = state
                    .reservations
                    .get_mut(&command.call_id)
                    .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
                reservation.day_start = command.day_start;
                reservation.month_start = command.month_start;
                reservation.updated_at = command.now;
                return Ok(ReservationOutcome::ReusedPrepared {
                    daily_used: state.daily_used,
                    monthly_used: state.monthly_used,
                });
            }
            return match existing.state {
                "dispatching" => Ok(ReservationOutcome::InProgress),
                "settled" | "unknown" | "provider_rejected" | "denied" => existing
                    .outcome
                    .clone()
                    .map(ReservationOutcome::Replay)
                    .ok_or(TavilyCreditBudgetRepositoryError::InvalidState),
                _ => Err(TavilyCreditBudgetRepositoryError::InvalidState),
            };
        }
        if let Some(denial) = reconcile(&mut state, &command) {
            return Ok(denied_outcome(denial, &state));
        }
        if state
            .daily_blocked_until
            .is_some_and(|until| until > command.now)
        {
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::PeriodResetUnconfirmed,
                    TavilyCreditBudgetScope::Daily,
                    state.daily_blocked_until,
                ),
                &state,
            ));
        }
        let reserved = command.operation.reservation();
        if state.daily_used + reserved > command.config.daily_limit() {
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    next_day_start(command.day_start),
                ),
                &state,
            ));
        }
        if state.monthly_used + reserved > command.config.monthly_limit() {
            return Ok(denied_outcome(
                denial(
                    TavilyCreditBudgetReason::MonthlyLimitExceeded,
                    TavilyCreditBudgetScope::Monthly,
                    next_month_start(command.month_start),
                ),
                &state,
            ));
        }
        state.daily_used += reserved;
        state.monthly_used += reserved;
        state.reservations.insert(
            command.call_id,
            TestReservation {
                fingerprint: command.request_fingerprint,
                operation: command.operation,
                state: "prepared",
                reserved,
                settled: None,
                day_start: command.day_start,
                month_start: command.month_start,
                outcome: None,
                updated_at: command.now,
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
        let mut state = self.0.lock().await;
        let day_start = command.now.date_naive();
        let month_start = first_of_month(day_start);
        state
            .reservations
            .entry(command.call_id.clone())
            .or_insert(TestReservation {
                fingerprint: command.request_fingerprint,
                operation: command.operation,
                state: "denied",
                reserved: Decimal::ZERO,
                settled: Some(Decimal::ZERO),
                day_start,
                month_start,
                outcome: Some(command.outcome),
                updated_at: command.now,
            });
        Ok(())
    }

    async fn record_cache_hit(
        &self,
        _transaction: &UnitOfWorkTransaction,
        command: crate::tavily_credit_budget::RecordCacheHitCommand,
    ) -> Result<ReservationClaim, TavilyCreditBudgetRepositoryError> {
        let mut state = self.0.lock().await;
        if let Some(existing) = state.reservations.get(&command.call_id).cloned() {
            if existing.fingerprint != command.request_fingerprint
                || existing.operation != command.operation
            {
                return Ok(ReservationClaim::Conflict);
            }
            return match existing.state {
                "prepared" => {
                    state.daily_used -= existing.reserved;
                    state.monthly_used -= existing.reserved;
                    let reservation = state
                        .reservations
                        .get_mut(&command.call_id)
                        .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
                    reservation.state = "settled";
                    reservation.settled = Some(Decimal::ZERO);
                    reservation.outcome = Some(command.outcome.clone());
                    reservation.updated_at = command.now;
                    Ok(ReservationClaim::Replay(command.outcome))
                }
                "dispatching" => Ok(ReservationClaim::InProgress),
                _ => existing
                    .outcome
                    .map(ReservationClaim::Replay)
                    .ok_or(TavilyCreditBudgetRepositoryError::InvalidState),
            };
        }
        state.reservations.insert(
            command.call_id,
            TestReservation {
                fingerprint: command.request_fingerprint,
                operation: command.operation,
                state: "settled",
                reserved: Decimal::ZERO,
                settled: Some(Decimal::ZERO),
                day_start: command.now.date_naive(),
                month_start: first_of_month(command.now.date_naive()),
                outcome: Some(command.outcome.clone()),
                updated_at: command.now,
            },
        );
        Ok(ReservationClaim::Replay(command.outcome))
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
        if reservation.state == "prepared" {
            reservation.state = "dispatching";
            return Ok(ReservationClaim::Claimed);
        }
        if reservation.state == "dispatching" {
            return Ok(ReservationClaim::InProgress);
        }
        reservation
            .outcome
            .clone()
            .map(ReservationClaim::Replay)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)
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
            .get(call_id)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        if reservation.fingerprint != request_fingerprint || reservation.state != "dispatching" {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        let adjustment = actual_credits - reservation.reserved;
        let day_carry = reservation.day_start < state.day_start.unwrap_or(reservation.day_start);
        let month_carry =
            reservation.month_start < state.month_start.unwrap_or(reservation.month_start);
        state.daily_used += adjustment;
        state.monthly_used += adjustment;
        if day_carry {
            state.daily_carry += adjustment;
        }
        if month_carry {
            state.monthly_carry += adjustment;
        }
        let reservation = state
            .reservations
            .get_mut(call_id)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState)?;
        reservation.state = "settled";
        reservation.settled = Some(actual_credits);
        reservation.outcome = Some(outcome);
        reservation.updated_at = _now;
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
            || reservation.state != "dispatching"
        {
            return Err(TavilyCreditBudgetRepositoryError::InvalidState);
        }
        reservation.state = match command.state {
            ReservationFailureState::Unknown => "unknown",
            ReservationFailureState::ProviderRejected => "provider_rejected",
        };
        reservation.outcome = Some(command.outcome);
        reservation.updated_at = command.now;
        Ok(())
    }
}

fn reservation_existing(
    reservation: &TestReservation,
) -> Result<ExistingReservation, TavilyCreditBudgetRepositoryError> {
    match reservation.state {
        "prepared" => Ok(ExistingReservation::Prepared),
        "dispatching" => Ok(ExistingReservation::InProgress),
        "settled" | "unknown" | "provider_rejected" | "denied" => reservation
            .outcome
            .clone()
            .map(ExistingReservation::Replay)
            .ok_or(TavilyCreditBudgetRepositoryError::InvalidState),
        _ => Err(TavilyCreditBudgetRepositoryError::InvalidState),
    }
}

fn reconcile(
    state: &mut TestState,
    command: &ReservationCommand,
) -> Option<TavilyCreditBudgetDenial> {
    if state.month_start.is_none() || state.day_start.is_none() {
        return Some(denial(
            TavilyCreditBudgetReason::BootstrapRequired,
            TavilyCreditBudgetScope::Account,
            None,
        ));
    }
    let current_month = state.month_start?;
    let current_day = state.day_start?;
    if command.month_start < current_month || command.day_start < current_day {
        return Some(denial(
            TavilyCreditBudgetReason::PeriodResetUnconfirmed,
            TavilyCreditBudgetScope::Confirmation,
            None,
        ));
    }
    let month_changed = command.month_start > current_month;
    if month_changed {
        if !(command.monthly_provider_usage < state.provider_usage
            || (state.provider_usage == Decimal::ZERO
                && command.monthly_provider_usage == Decimal::ZERO))
        {
            return Some(denial(
                TavilyCreditBudgetReason::PeriodResetUnconfirmed,
                TavilyCreditBudgetScope::Monthly,
                next_month_start(current_month),
            ));
        }
        let boundary = Utc.from_utc_datetime(&command.month_start.and_hms_opt(0, 0, 0)?);
        let carry = state
            .reservations
            .values()
            .filter(|reservation| {
                reservation.month_start < command.month_start
                    && (matches!(
                        reservation.state,
                        "prepared" | "dispatching" | "unknown" | "provider_rejected"
                    ) || (reservation.state == "settled" && reservation.updated_at >= boundary))
            })
            .map(|reservation| reservation.settled.unwrap_or(reservation.reserved))
            .sum::<Decimal>();
        state.month_start = Some(command.month_start);
        state.monthly_used = command.monthly_provider_usage + carry;
        state.monthly_carry = carry;
        state.provider_usage = command.monthly_provider_usage;
    } else if command.monthly_provider_usage < state.provider_usage {
        return Some(denial(
            TavilyCreditBudgetReason::PeriodResetUnconfirmed,
            TavilyCreditBudgetScope::Monthly,
            next_month_start(current_month),
        ));
    }
    if command.day_start > current_day {
        let boundary = Utc.from_utc_datetime(&command.day_start.and_hms_opt(0, 0, 0)?);
        let pending_carry = state
            .reservations
            .values()
            .filter(|reservation| {
                reservation.day_start < command.day_start
                    && (matches!(
                        reservation.state,
                        "prepared" | "dispatching" | "unknown" | "provider_rejected"
                    ) || (reservation.state == "settled" && reservation.updated_at >= boundary))
            })
            .map(|reservation| reservation.settled.unwrap_or(reservation.reserved))
            .sum::<Decimal>();
        let unaccounted_provider_usage = if month_changed {
            command.monthly_provider_usage
        } else {
            (command.monthly_provider_usage - state.day_provider_usage - state.daily_used)
                .max(Decimal::ZERO)
        };
        let carry = pending_carry + unaccounted_provider_usage;
        state.day_start = Some(command.day_start);
        state.daily_used = carry;
        state.daily_carry = carry;
        state.day_provider_usage = command.monthly_provider_usage;
        state.daily_blocked_until = None;
    }
    let monthly_in_flight = state
        .reservations
        .values()
        .filter(|reservation| {
            reservation.month_start == command.month_start
                && matches!(
                    reservation.state,
                    "prepared" | "dispatching" | "unknown" | "provider_rejected"
                )
        })
        .map(|reservation| reservation.reserved)
        .sum::<Decimal>();
    state.monthly_used = state.monthly_carry
        + (state.monthly_used - state.monthly_carry)
            .max(command.monthly_provider_usage + monthly_in_flight);
    let daily_delta = command.monthly_provider_usage - state.day_provider_usage;
    let daily_in_flight = state
        .reservations
        .values()
        .filter(|reservation| {
            reservation.day_start == command.day_start
                && matches!(
                    reservation.state,
                    "prepared" | "dispatching" | "unknown" | "provider_rejected"
                )
        })
        .map(|reservation| reservation.reserved)
        .sum::<Decimal>();
    state.daily_used = state.daily_carry
        + (state.daily_used - state.daily_carry).max(daily_delta + daily_in_flight);
    state.provider_usage = command.monthly_provider_usage;
    None
}

fn denied_outcome(denial: TavilyCreditBudgetDenial, state: &TestState) -> ReservationOutcome {
    ReservationOutcome::Denied {
        denial,
        daily_used: state.daily_used,
        monthly_used: state.monthly_used,
    }
}

fn denial(
    reason: TavilyCreditBudgetReason,
    scope: TavilyCreditBudgetScope,
    next_reset: Option<DateTime<Utc>>,
) -> TavilyCreditBudgetDenial {
    TavilyCreditBudgetDenial::new(reason, scope, next_reset)
}

fn next_day_start(day: NaiveDate) -> Option<DateTime<Utc>> {
    let next = day.succ_opt()?;
    Some(Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0)?))
}

fn next_month_start(day: NaiveDate) -> Option<DateTime<Utc>> {
    let next = if day.month() == 12 {
        NaiveDate::from_ymd_opt(day.year().checked_add(1)?, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(day.year(), day.month() + 1, 1)?
    };
    Some(Utc.from_utc_datetime(&next.and_hms_opt(0, 0, 0)?))
}

fn first_of_month(day: NaiveDate) -> NaiveDate {
    day.with_day(1).unwrap_or(day)
}

fn make_use_cases(
    repository: TestRepository,
    provider: TestProvider,
    clock: TestClock,
    config: Option<TavilyCreditBudgetConfig>,
) -> TavilyCreditBudgetUseCases {
    make_use_cases_with_cache(
        repository,
        provider,
        clock,
        config,
        Arc::new(TestRetrievalCache),
    )
}

fn make_use_cases_with_cache(
    repository: TestRepository,
    provider: TestProvider,
    clock: TestClock,
    config: Option<TavilyCreditBudgetConfig>,
    retrieval_cache: SharedTavilyRetrievalCacheRepository,
) -> TavilyCreditBudgetUseCases {
    TavilyCreditBudgetUseCases::new(
        Arc::new(TestUnitOfWork),
        Arc::new(repository),
        retrieval_cache,
        Some(Arc::new(provider)),
        config,
        Some(TavilyCredentialGeneration::from_secret(
            b"synthetic-test-key",
        )),
        Arc::new(clock),
    )
}

fn free_usage(plan_credits_used: Decimal) -> WebSearchAccountUsage {
    WebSearchAccountUsage {
        current_plan: "Researcher".into(),
        plan_credits_used,
        paygo_credits_used: Decimal::ZERO,
    }
}

fn test_config() -> TavilyCreditBudgetConfig {
    TavilyCreditBudgetConfig::free_tier("example-account".into(), true)
        .expect("valid synthetic configuration")
}

fn fingerprint(value: char) -> String {
    value.to_string().repeat(64)
}

async fn bootstrapped(
    provider: TestProvider,
    initial_usage: Decimal,
) -> (
    TavilyCreditBudgetUseCases,
    TestRepository,
    TestClock,
    TestProvider,
) {
    bootstrapped_with_cache(provider, initial_usage, Arc::new(TestRetrievalCache)).await
}

async fn bootstrapped_with_cache(
    provider: TestProvider,
    initial_usage: Decimal,
    retrieval_cache: SharedTavilyRetrievalCacheRepository,
) -> (
    TavilyCreditBudgetUseCases,
    TestRepository,
    TestClock,
    TestProvider,
) {
    provider.set_usage(Ok(free_usage(initial_usage)));
    let repository = TestRepository::default();
    let clock = TestClock::at(utc(2026, 1, 4, 23, 59));
    let use_cases = make_use_cases_with_cache(
        repository.clone(),
        provider.clone(),
        clock.clone(),
        Some(test_config()),
        retrieval_cache,
    );
    use_cases.bootstrap().await.expect("bootstrap budget");
    clock.set(utc(2026, 1, 5, 0, 0));
    (use_cases, repository, clock, provider)
}

#[tokio::test]
async fn search_cache_hit_skips_provider_and_credit_usage_until_ttl_expires() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO));
    let (use_cases, repository, clock, provider) = bootstrapped_with_cache(
        provider,
        Decimal::ZERO,
        Arc::new(MemoryTestRetrievalCache::default()),
    )
    .await;
    let first = use_cases
        .search(
            "cache-miss-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let state_after_miss = {
        let state = repository.0.lock().await;
        (state.daily_used, state.monthly_used)
    };
    let second = use_cases
        .search(
            "cache-hit-call".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let (state_after_hit, cache_hit_reservation) = {
        let state = repository.0.lock().await;
        let reservation = state.reservations.get("cache-hit-call");
        (
            (state.daily_used, state.monthly_used),
            reservation
                .map(|reservation| (reservation.state, reservation.reserved, reservation.settled)),
        )
    };

    clock.set(utc(2026, 1, 5, 0, 15));
    let third = use_cases
        .search(
            "cache-expiry-call".into(),
            fingerprint('c'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let state_after_expiry = {
        let state = repository.0.lock().await;
        (state.daily_used, state.monthly_used)
    };
    let result = (
        first,
        second,
        third,
        provider.call_counts(),
        state_after_miss,
        state_after_hit,
        cache_hit_reservation,
        state_after_expiry,
    );
    let expected_results = vec![WebSearchResult {
        title: "Example result".into(),
        url: "https://example.invalid/article".into(),
        published_date: None,
        snippet: "Example excerpt".into(),
    }];

    assert_eq!(
        result,
        (
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: expected_results.clone(),
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: expected_results.clone(),
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: expected_results,
                fetched_at: Some(utc(2026, 1, 5, 0, 15)),
            }),
            (3, 2, 0),
            (Decimal::ONE, Decimal::ONE),
            (Decimal::ONE, Decimal::ONE),
            Some(("settled", Decimal::ZERO, Some(Decimal::ZERO))),
            (Decimal::new(2, 0), Decimal::new(2, 0)),
        ),
    );
}

#[tokio::test]
async fn extraction_cache_is_valid_until_the_24_hour_expiry() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO));
    let (use_cases, _repository, clock, provider) = bootstrapped_with_cache(
        provider,
        Decimal::ZERO,
        Arc::new(MemoryTestRetrievalCache::default()),
    )
    .await;
    let first = use_cases
        .extract_page(
            "extract-cache-miss".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;
    clock.set(utc(2026, 1, 5, 23, 59));
    let second = use_cases
        .extract_page(
            "extract-cache-hit".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;
    clock.set(utc(2026, 1, 6, 0, 0));
    let third = use_cases
        .extract_page(
            "extract-cache-expiry".into(),
            fingerprint('c'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;

    assert_eq!(
        (first, second, third, provider.call_counts()),
        (
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: "Example page content".into(),
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: "Example page content".into(),
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: "Example page content".into(),
                fetched_at: Some(utc(2026, 1, 6, 0, 0)),
            }),
            (3, 0, 2),
        ),
    );
}

#[tokio::test]
async fn empty_extractions_are_fetched_again_instead_of_cached() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO)).with_extract_response(Ok(
        WebPageExtractionResponse {
            raw_content: Some(String::new()),
            failure: None,
            credits_used: Some(Decimal::ONE),
        },
    ));
    let (use_cases, repository, clock, provider) = bootstrapped_with_cache(
        provider,
        Decimal::ZERO,
        Arc::new(MemoryTestRetrievalCache::default()),
    )
    .await;
    let first = use_cases
        .extract_page(
            "empty-first-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;
    clock.set(utc(2026, 1, 5, 0, 1));
    let second = use_cases
        .extract_page(
            "empty-second-call".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;
    let state = repository.0.lock().await;

    assert_eq!(
        (
            first,
            second,
            provider.call_counts(),
            (state.daily_used, state.monthly_used),
        ),
        (
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: String::new(),
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: String::new(),
                fetched_at: Some(utc(2026, 1, 5, 0, 1)),
            }),
            (3, 0, 2),
            (Decimal::new(2, 0), Decimal::new(2, 0)),
        ),
    );
}

#[tokio::test]
async fn failed_extractions_are_fetched_again_instead_of_cached() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO)).with_extract_response(Ok(
        WebPageExtractionResponse {
            raw_content: None,
            failure: Some(WebPageExtractionFailure {
                url: "https://example.invalid/article?section=one".into(),
                message: "synthetic extraction failure".into(),
            }),
            credits_used: Some(Decimal::ZERO),
        },
    ));
    let (use_cases, repository, _clock, provider) = bootstrapped_with_cache(
        provider,
        Decimal::ZERO,
        Arc::new(MemoryTestRetrievalCache::default()),
    )
    .await;
    let first = use_cases
        .extract_page(
            "failed-first-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;
    let second = use_cases
        .extract_page(
            "failed-second-call".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "https://example.invalid/article?section=one".into(),
        )
        .await;
    let state = repository.0.lock().await;
    let expected_error =
        TavilyCreditBudgetUseCasesError::Provider(WebSearchError::ExtractionFailed {
            url: "https://example.invalid/article?section=one".into(),
            message: "synthetic extraction failure".into(),
        });

    assert_eq!(
        (
            first,
            second,
            provider.call_counts(),
            (state.daily_used, state.monthly_used),
        ),
        (
            Err(expected_error.clone()),
            Err(expected_error),
            (3, 0, 2),
            (Decimal::ZERO, Decimal::ZERO),
        ),
    );
}

fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
        .single()
        .expect("valid test date")
}

#[tokio::test]
async fn bootstrap_restores_monthly_usage_and_blocks_until_next_utc_day() {
    let provider = TestProvider::new(free_usage(Decimal::new(40, 0)));
    let repository = TestRepository::default();
    let clock = TestClock::at(utc(2026, 1, 4, 23, 59));
    let use_cases = make_use_cases(
        repository.clone(),
        provider.clone(),
        clock.clone(),
        Some(test_config()),
    );
    use_cases.bootstrap().await.expect("bootstrap budget");

    let denial = use_cases
        .search(
            "call-one".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (
            denial,
            provider.call_counts(),
            repository.0.lock().await.monthly_used,
        ),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::PeriodResetUnconfirmed,
                    TavilyCreditBudgetScope::Daily,
                    Some(utc(2026, 1, 5, 0, 0)),
                ),
            )),
            (2, 0, 0),
            Decimal::new(40, 0),
        ),
    );
}

#[tokio::test]
async fn same_call_reuses_settled_response_after_restart_and_rejects_changed_request() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO));
    let (use_cases, repository, clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    let result = use_cases
        .search(
            "call-one".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let restarted = make_use_cases(repository, provider.clone(), clock, Some(test_config()));
    let replay = restarted
        .search(
            "call-one".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let conflict = restarted
        .search(
            "call-one".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "different query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (result.clone(), replay, conflict, provider.call_counts()),
        (
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: vec![WebSearchResult {
                    title: "Example result".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: None,
                    snippet: "Example excerpt".into(),
                }],
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            result,
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::IdempotencyConflict,
                    TavilyCreditBudgetScope::Account,
                    None,
                ),
            )),
            (2, 1, 0),
        ),
    );
}

#[rstest::rstest]
#[case::paid_plan("Paid", Decimal::ZERO, TavilyCreditBudgetReason::AccountPlanNotFree)]
#[case::paygo_usage(
    "Researcher",
    Decimal::ONE,
    TavilyCreditBudgetReason::PaygoUsageDetected
)]
#[tokio::test]
async fn paid_plan_or_paygo_usage_stops_before_search(
    #[case] current_plan: &str,
    #[case] paygo_credits_used: Decimal,
    #[case] expected_reason: TavilyCreditBudgetReason,
) {
    let provider = TestProvider::new(free_usage(Decimal::ZERO));
    let (use_cases, _repository, _clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    provider.set_usage(Ok(WebSearchAccountUsage {
        current_plan: current_plan.into(),
        plan_credits_used: Decimal::ZERO,
        paygo_credits_used,
    }));
    let result = use_cases
        .search(
            "call-one".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (result, provider.call_counts()),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    expected_reason,
                    TavilyCreditBudgetScope::Account,
                    None,
                ),
            )),
            (2, 0, 0),
        ),
    );
}

#[rstest::rstest]
#[case::usage_endpoint_failure(Err(WebSearchError::Usage("synthetic failure".into())))]
#[case::negative_usage(Ok(WebSearchAccountUsage {
    current_plan: "Researcher".into(),
    plan_credits_used: Decimal::new(-1, 0),
    paygo_credits_used: Decimal::ZERO,
}))]
#[case::excess_precision(Ok(WebSearchAccountUsage {
    current_plan: "Researcher".into(),
    plan_credits_used: Decimal::new(1, 7),
    paygo_credits_used: Decimal::ZERO,
}))]
#[tokio::test]
async fn unavailable_or_invalid_account_usage_stops_before_search(
    #[case] usage: Result<WebSearchAccountUsage, WebSearchError>,
) {
    let provider = TestProvider::new(free_usage(Decimal::ZERO));
    let (use_cases, _repository, _clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    provider.set_usage(usage);
    let result = use_cases
        .search(
            "call-one".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (result, provider.call_counts()),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::UsageConfirmationFailed,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            (2, 0, 0),
        ),
    );
}

#[tokio::test]
async fn provider_timeout_and_missing_usage_keep_the_reservation_and_never_retry() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO))
        .with_search_response(Err(WebSearchError::Network("synthetic timeout".into())));
    let (use_cases, repository, clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    let timeout = use_cases
        .search(
            "timeout-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let restarted = make_use_cases(
        repository.clone(),
        provider.clone(),
        clock.clone(),
        Some(test_config()),
    );
    let retry_timeout = restarted
        .search(
            "timeout-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (timeout, retry_timeout, provider.call_counts()),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            (2, 1, 0),
        ),
    );

    let provider =
        TestProvider::new(free_usage(Decimal::ZERO)).with_search_response(Ok(WebSearchResponse {
            results: Vec::new(),
            credits_used: None,
        }));
    let (use_cases, repository, clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    let result = use_cases
        .search(
            "missing-usage".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let restarted = make_use_cases(repository, provider.clone(), clock, Some(test_config()));
    let retry = restarted
        .search(
            "missing-usage".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (result, retry, provider.call_counts()),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::UsageConfirmationFailed,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::UsageConfirmationFailed,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            (2, 1, 0),
        ),
    );
}

#[tokio::test]
async fn provider_rejection_is_structured_and_same_call_does_not_retry() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO)).with_search_response(Err(
        WebSearchError::Api {
            status: 429,
            message: "synthetic rejection".into(),
        },
    ));
    let (use_cases, repository, clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    let rejected = use_cases
        .search(
            "rejected-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let restarted = make_use_cases(repository, provider.clone(), clock, Some(test_config()));
    let replay = restarted
        .search(
            "rejected-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (rejected.clone(), replay, provider.call_counts()),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::ProviderRejected,
                    TavilyCreditBudgetScope::Provider,
                    None,
                )
                .with_provider_status(429),
            )),
            rejected,
            (2, 1, 0),
        ),
    );
}

#[tokio::test]
async fn external_usage_and_monthly_threshold_deny_before_search() {
    let provider = TestProvider::new(free_usage(Decimal::new(899, 0)));
    let (use_cases, _repository, _clock, provider) =
        bootstrapped(provider, Decimal::new(899, 0)).await;
    let allowed_at_threshold = use_cases
        .search(
            "last-credit".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    provider.set_usage(Ok(free_usage(Decimal::new(900, 0))));
    let denied_after_external_usage = use_cases
        .search(
            "over-limit".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    assert_eq!(
        (
            allowed_at_threshold,
            denied_after_external_usage,
            provider.call_counts(),
        ),
        (
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: vec![WebSearchResult {
                    title: "Example result".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: None,
                    snippet: "Example excerpt".into(),
                }],
                fetched_at: Some(utc(2026, 1, 5, 0, 0)),
            }),
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::MonthlyLimitExceeded,
                    TavilyCreditBudgetScope::Monthly,
                    Some(utc(2026, 2, 1, 0, 0)),
                ),
            )),
            (3, 1, 0),
        ),
    );
}

#[tokio::test]
async fn daily_boundary_carries_unknown_credit_and_unmatched_provider_usage() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO))
        .with_search_response(Err(WebSearchError::Network("synthetic timeout".into())));
    let (use_cases, repository, clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    let unknown = use_cases
        .search(
            "unknown-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    clock.set(utc(2026, 1, 6, 0, 0));
    provider.set_usage(Ok(free_usage(Decimal::new(25, 0))));
    let denied = use_cases
        .search(
            "next-day-call".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let state = repository.0.lock().await;
    assert_eq!(
        (
            unknown,
            denied,
            provider.call_counts(),
            state.daily_used,
            state.daily_carry,
        ),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    Some(utc(2026, 1, 7, 0, 0)),
                ),
            )),
            (3, 1, 0),
            Decimal::new(25, 0),
            Decimal::new(25, 0),
        ),
    );
}

#[tokio::test]
async fn monthly_reset_carries_unknown_credit_when_provider_usage_increases() {
    let provider = TestProvider::new(free_usage(Decimal::new(899, 0)))
        .with_search_response(Err(WebSearchError::Network("synthetic timeout".into())));
    let (use_cases, repository, clock, provider) =
        bootstrapped(provider, Decimal::new(899, 0)).await;
    let unknown = use_cases
        .search(
            "unknown-call".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    provider.set_search_response(Ok(WebSearchResponse {
        results: vec![WebSearchResult {
            title: "Example result".into(),
            url: "https://example.invalid/article".into(),
            published_date: None,
            snippet: "Example excerpt".into(),
        }],
        credits_used: Some(Decimal::ONE),
    }));
    provider.set_usage(Ok(free_usage(Decimal::ZERO)));
    clock.set(utc(2026, 2, 1, 0, 0));
    let after_reset = use_cases
        .search(
            "after-reset".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    provider.set_usage(Ok(free_usage(Decimal::new(898, 0))));
    let after_external_usage = use_cases
        .search(
            "after-external-usage".into(),
            fingerprint('c'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;
    let state = repository.0.lock().await;
    assert_eq!(
        (
            unknown,
            after_reset,
            after_external_usage,
            provider.call_counts(),
            state.monthly_used,
            state.monthly_carry,
        ),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::OutcomeUnknown,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            Ok(crate::tavily_credit_budget::TavilyRetrieved {
                value: vec![WebSearchResult {
                    title: "Example result".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: None,
                    snippet: "Example excerpt".into(),
                }],
                fetched_at: Some(utc(2026, 2, 1, 0, 0)),
            }),
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    Some(utc(2026, 2, 2, 0, 0)),
                ),
            )),
            (4, 2, 0),
            Decimal::new(899, 0),
            Decimal::ONE,
        ),
    );
}

#[rstest]
#[case::above_reserved(Decimal::new(2, 0))]
#[case::zero_for_basic_search(Decimal::ZERO)]
#[tokio::test]
async fn invalid_search_usage_is_unknown(#[case] invalid_search_usage: Decimal) {
    let provider =
        TestProvider::new(free_usage(Decimal::ZERO)).with_search_response(Ok(WebSearchResponse {
            results: Vec::new(),
            credits_used: Some(invalid_search_usage),
        }));
    let (use_cases, _repository, _clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    let invalid_usage = use_cases
        .search(
            "invalid-usage".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        )
        .await;

    assert_eq!(
        (invalid_usage, provider.call_counts()),
        (
            Err(TavilyCreditBudgetUseCasesError::Denied(
                TavilyCreditBudgetDenial::new(
                    TavilyCreditBudgetReason::UsageConfirmationFailed,
                    TavilyCreditBudgetScope::Confirmation,
                    None,
                ),
            )),
            (2, 1, 0),
        ),
    );
}

#[tokio::test]
async fn extract_failure_settles_provider_reported_credits() {
    let failed_provider = TestProvider::new(free_usage(Decimal::ZERO)).with_extract_response(Ok(
        WebPageExtractionResponse {
            raw_content: None,
            failure: Some(WebPageExtractionFailure {
                url: "https://example.invalid/article".into(),
                message: "synthetic extraction failure".into(),
            }),
            credits_used: Some(Decimal::ZERO),
        },
    ));
    let (use_cases, repository, _clock, failed_provider) =
        bootstrapped(failed_provider, Decimal::ZERO).await;
    let extraction = use_cases
        .extract_page(
            "extract-call".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "https://example.invalid/article".into(),
        )
        .await;
    let state = repository.0.lock().await;

    assert_eq!(
        (
            extraction,
            failed_provider.call_counts(),
            state.daily_used,
            state.monthly_used,
        ),
        (
            Err(TavilyCreditBudgetUseCasesError::Provider(
                WebSearchError::ExtractionFailed {
                    url: "https://example.invalid/article".into(),
                    message: "synthetic extraction failure".into(),
                },
            )),
            (2, 0, 1),
            Decimal::ZERO,
            Decimal::ZERO,
        ),
    );
}

#[tokio::test]
async fn concurrent_reservations_cannot_pass_the_daily_limit() {
    let provider = TestProvider::new(free_usage(Decimal::ZERO));
    let (first_use_case, repository, clock, provider) = bootstrapped(provider, Decimal::ZERO).await;
    repository
        .seed_usage(clock.now(), Decimal::new(24, 0), Decimal::new(24, 0))
        .await;
    let second_use_case = make_use_cases(repository, provider.clone(), clock, Some(test_config()));
    let (first, second) = tokio::join!(
        first_use_case.search(
            "parallel-one".into(),
            fingerprint('a'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        ),
        second_use_case.search(
            "parallel-two".into(),
            fingerprint('b'),
            "synthetic-scope".into(),
            "synthetic query".into(),
            None,
            None,
        ),
    );
    let mut outcomes = [first, second]
        .map(|result| match result {
            Ok(results) => Ok(results.value),
            Err(TavilyCreditBudgetUseCasesError::Denied(denial)) => {
                Err((denial.reason, denial.scope, denial.next_reset))
            }
            Err(_) => Err((
                TavilyCreditBudgetReason::UsageConfirmationFailed,
                TavilyCreditBudgetScope::Confirmation,
                None,
            )),
        })
        .into_iter()
        .collect::<Vec<_>>();
    outcomes.sort_by_key(|outcome| match outcome {
        Ok(_) => 0,
        Err(_) => 1,
    });
    assert_eq!(
        (outcomes, provider.call_counts()),
        (
            vec![
                Ok(vec![WebSearchResult {
                    title: "Example result".into(),
                    url: "https://example.invalid/article".into(),
                    published_date: None,
                    snippet: "Example excerpt".into(),
                }]),
                Err((
                    TavilyCreditBudgetReason::DailyLimitExceeded,
                    TavilyCreditBudgetScope::Daily,
                    Some(utc(2026, 1, 6, 0, 0)),
                )),
            ],
            (3, 1, 0),
        ),
    );
}
