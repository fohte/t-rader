use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Timelike, Utc};

use super::super::error::{BarsUseCaseError, UsStockBarsIngestError};
use super::super::types::{
    UsStockBarsIngestResult, UsStockBarsIngestStats, UsStockSipBarRefetchConfig,
    UsStockSipBarRefetchCursor, UsStockSipBarRefetchTarget,
};
use super::super::us_stock_source::UsStockBarSource;
use super::super::use_cases::UsStockBarsIngestUseCases;
use super::super::write_throttle::WriteThrottle;
use super::request_budget::RequestBudget;
use super::{
    BatchIngestContext, IngestContext, SIP_REFETCH_MAX_ROWS_PER_RUN, SipRefetchLimits,
    UsStockBarRange, UsStockBarTimeframe,
};

const MAX_REQUESTS_PER_RUN: usize = usize::MAX;
const START_MINUTE: u32 = 50;
const RUN_DEADLINE_BUFFER: Duration = Duration::from_secs(5 * 60);
const HOURLY_HISTORY_START: (i32, u32, u32) = (2016, 1, 1);

#[derive(Clone, Copy)]
struct UsStockSipBarRefetchRanges {
    daily: UsStockBarRange,
    hourly: UsStockBarRange,
    minute: UsStockBarRange,
}

struct UsStockSipBarRefetchFrame {
    instrument_id: String,
    timeframe: UsStockBarTimeframe,
    range: UsStockBarRange,
    page_token: Option<String>,
    resume_after: Option<UsStockSipBarRefetchCursor>,
}

impl UsStockBarsIngestUseCases {
    pub async fn refetch_us_stock_bars_via_sip(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
        config: UsStockSipBarRefetchConfig,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        let mut budget = RequestBudget::new(
            MAX_REQUESTS_PER_RUN,
            run_duration_until_next_start(now),
            self.clock.instant(),
        );
        let mut stats = UsStockBarsIngestStats::default();
        if !budget.can_start_request(self.clock.instant()) {
            stats.interrupted_reason = budget.interrupted_reason.map(str::to_owned);
            return Ok(UsStockBarsIngestResult::new(stats, None));
        }
        let _lease_guard = match self
            .us_stock_sip_refetch_lease
            .acquire(Duration::ZERO)
            .await
        {
            Ok(Some(guard)) => guard,
            Ok(None) => {
                stats.interrupted_reason = Some("sip_refetch_lease_busy".to_owned());
                return Ok(UsStockBarsIngestResult::new(stats, None));
            }
            Err(error) => {
                return Err(UsStockBarsIngestError::new(
                    UsStockBarsIngestResult::new(stats, None),
                    error.into(),
                ));
            }
        };

        let mut context = IngestContext {
            budget,
            stats: UsStockBarsIngestStats::default(),
            write_throttle: WriteThrottle::default(),
            sip_refetch: Some(SipRefetchLimits {
                max_rows_per_hour: config.max_rows_per_hour,
                max_rows_per_run: SIP_REFETCH_MAX_ROWS_PER_RUN,
            }),
        };
        let refetch_result = self
            .refetch_pending_targets(source, now, config, &mut context)
            .await;
        match refetch_result {
            Ok(completed) => {
                if !completed && context.stats.interrupted_reason.is_none() {
                    context.stats.interrupted_reason =
                        context.budget.interrupted_reason.map(str::to_owned);
                }
                Ok(UsStockBarsIngestResult::new(context.stats, None))
            }
            Err(error) => Err(UsStockBarsIngestError::new(
                UsStockBarsIngestResult::new(context.stats, None),
                error,
            )),
        }
    }

    async fn refetch_pending_targets(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
        config: UsStockSipBarRefetchConfig,
        context: &mut IngestContext,
    ) -> Result<bool, BarsUseCaseError> {
        let targets = self
            .bars
            .repository
            .find_pending_us_stock_sip_bar_refetch_targets(config.max_symbols)
            .await?;
        context.stats.symbols_attempted = targets.len();

        let ranges = UsStockSipBarRefetchRanges {
            daily: UsStockBarRange {
                from: UsStockBarTimeframe::Daily.history_start(now),
                to: UsStockBarTimeframe::Daily.history_end(now),
            },
            hourly: UsStockBarRange {
                from: Utc
                    .with_ymd_and_hms(
                        HOURLY_HISTORY_START.0,
                        HOURLY_HISTORY_START.1,
                        HOURLY_HISTORY_START.2,
                        0,
                        0,
                        0,
                    )
                    .single()
                    .unwrap_or(now),
                to: UsStockBarTimeframe::Hourly.history_end(now),
            },
            minute: UsStockBarRange {
                from: UsStockBarTimeframe::Minute.history_start(now),
                to: UsStockBarTimeframe::Minute.history_end(now),
            },
        };

        for target in targets {
            if !self
                .refetch_target(source, &target, ranges, context)
                .await?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn refetch_target(
        &self,
        source: &dyn UsStockBarSource,
        refetch_target: &UsStockSipBarRefetchTarget,
        ranges: UsStockSipBarRefetchRanges,
        context: &mut IngestContext,
    ) -> Result<bool, BarsUseCaseError> {
        let cursor = refetch_target.cursor.as_ref();
        let target = &refetch_target.bars;
        let mut frames = [
            (
                UsStockBarTimeframe::Daily,
                target.earliest_daily_bar,
                ranges.daily,
            ),
            (
                UsStockBarTimeframe::Hourly,
                target.earliest_hourly_bar,
                ranges.hourly,
            ),
            (
                UsStockBarTimeframe::Minute,
                target.earliest_minute_bar,
                ranges.minute,
            ),
        ];
        let start_index = cursor
            .and_then(|cursor| {
                frames.iter().position(|(timeframe, _, _)| {
                    timeframe.as_sip_refetch_timeframe() == cursor.timeframe
                })
            })
            .unwrap_or(0);
        if let Some(cursor) = cursor {
            frames[start_index].2 = UsStockBarRange {
                from: cursor.range_from,
                to: cursor.range_to,
            };
        }
        let active_frames = frames
            .iter()
            .enumerate()
            .filter(|(_, (_, earliest_bar, _))| earliest_bar.is_some())
            .collect::<Vec<_>>();

        for (active_index, (frame_index, (timeframe, _, range))) in active_frames.iter().enumerate()
        {
            if *frame_index < start_index {
                continue;
            }
            let resume_after =
                active_frames
                    .get(active_index + 1)
                    .map(|(_, (timeframe, _, range))| UsStockSipBarRefetchCursor {
                        timeframe: timeframe.as_sip_refetch_timeframe(),
                        range_from: range.from,
                        range_to: range.to,
                        page_token: None,
                    });
            let page_token = cursor.and_then(|cursor| {
                (cursor.timeframe == timeframe.as_sip_refetch_timeframe())
                    .then(|| cursor.page_token.clone())
                    .flatten()
            });
            let frame_result = self
                .ingest_refetch_frame(
                    source,
                    UsStockSipBarRefetchFrame {
                        instrument_id: target.instrument_id.clone(),
                        timeframe: *timeframe,
                        range: *range,
                        page_token,
                        resume_after,
                    },
                    context,
                )
                .await?;
            match frame_result {
                RefetchFrameResult::Interrupted => return Ok(false),
                RefetchFrameResult::Skipped(reason) => {
                    self.bars
                        .repository
                        .mark_us_stock_sip_bar_refetch_skipped(&target.instrument_id, &reason)
                        .await?;
                    return Ok(true);
                }
                RefetchFrameResult::Completed => {}
            }
            if *timeframe == UsStockBarTimeframe::Hourly {
                self.mark_hourly_history_completed(
                    std::slice::from_ref(&target.instrument_id),
                    range.to,
                    &Default::default(),
                )
                .await?;
            }
        }

        self.bars
            .repository
            .mark_us_stock_sip_bar_refetch_completed(&target.instrument_id)
            .await?;
        Ok(true)
    }

    async fn ingest_refetch_frame(
        &self,
        source: &dyn UsStockBarSource,
        frame: UsStockSipBarRefetchFrame,
        context: &mut IngestContext,
    ) -> Result<RefetchFrameResult, BarsUseCaseError> {
        let instrument_ids = [frame.instrument_id.clone()];
        let (completed, skipped_instrument_ids) = {
            let mut batch_context = BatchIngestContext::new(context);
            batch_context.page_token = frame.page_token;
            batch_context.sip_refetch_progress = Some(super::SipRefetchProgressContext {
                instrument_id: frame.instrument_id.clone(),
                resume_after: frame.resume_after,
            });
            let completed = self
                .ingest_batch(
                    source,
                    &instrument_ids,
                    frame.timeframe,
                    frame.range,
                    &mut batch_context,
                )
                .await?;
            (completed.completed, batch_context.skipped_instrument_ids)
        };
        if !completed {
            return Ok(RefetchFrameResult::Interrupted);
        }

        if !skipped_instrument_ids.contains(&frame.instrument_id) {
            return Ok(RefetchFrameResult::Completed);
        }
        let reason = context
            .stats
            .skipped_instruments
            .iter()
            .find(|skipped| skipped.instrument_id == frame.instrument_id)
            .map(|skipped| skipped.reason.clone())
            .unwrap_or_else(|| "Alpaca rejected the instrument".to_owned());
        Ok(RefetchFrameResult::Skipped(reason))
    }
}

fn run_duration_until_next_start(now: DateTime<Utc>) -> Duration {
    let current_hour_start = now
        .date_naive()
        .and_hms_opt(now.hour(), START_MINUTE, 0)
        .map(|time| time.and_utc())
        .unwrap_or(now);
    let next_start = if current_hour_start > now {
        current_hour_start
    } else {
        current_hour_start
            .checked_add_signed(ChronoDuration::hours(1))
            .unwrap_or(now)
    };
    let latest_finish = next_start
        .checked_sub_signed(ChronoDuration::from_std(RUN_DEADLINE_BUFFER).unwrap_or_default())
        .unwrap_or(now);
    latest_finish
        .signed_duration_since(now)
        .to_std()
        .unwrap_or_default()
}

enum RefetchFrameResult {
    Completed,
    Interrupted,
    Skipped(String),
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::{
            Arc, Mutex as StdMutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    use async_trait::async_trait;
    use chrono::{DateTime, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::{
        bars::clock::BarsIngestClock,
        bars::{
            BarsUseCases, SharedBarsRepository, SharedSipRefetchLease, SharedUsStockBarsWriteLock,
            SkippedUsStockInstrument, UsStockBarPage, UsStockBarQuery, UsStockBarSource,
            UsStockBarSourceError, UsStockBarsIngestStats, UsStockBarsIngestUseCases,
            UsStockBarsWriteLock, UsStockBarsWriteLockGuard, UsStockSipBarRefetchConfig,
            UsStockSipBarRefetchCursor, UsStockSipBarRefetchTimeframe, UsStockSplitBatch,
            UsStockSplitQuery, us_stock_sip_bars_history_cutoff,
        },
        persistence::PersistenceError,
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork},
    };

    use super::super::{
        BatchIngestContext, IngestContext, SIP_REFETCH_MAX_ROWS_PER_RUN, SipRefetchLimits,
        SipRefetchProgressContext, UsStockBarRange, UsStockBarTimeframe,
    };
    use super::{RequestBudget, WriteThrottle};

    #[derive(Default)]
    struct FakeSource {
        calls: Mutex<Vec<UsStockBarQuery>>,
        page_limits: Mutex<Vec<Option<u32>>>,
        bars: Mutex<Vec<Bar>>,
        skipped_instruments: Mutex<HashMap<String, String>>,
        scripted_pages: Mutex<Vec<UsStockBarPage>>,
    }

    struct FakeClock {
        started_at: Instant,
        started_at_utc: DateTime<Utc>,
        elapsed: StdMutex<Duration>,
        sleeps: StdMutex<Vec<Duration>>,
        write_lock_held_during_sleep: StdMutex<Vec<bool>>,
        active_write_locks: Arc<AtomicUsize>,
    }

    impl FakeClock {
        fn new(active_write_locks: Arc<AtomicUsize>) -> Self {
            Self {
                started_at: Instant::now(),
                started_at_utc: Utc::now(),
                elapsed: StdMutex::default(),
                sleeps: StdMutex::default(),
                write_lock_held_during_sleep: StdMutex::default(),
                active_write_locks,
            }
        }

        fn elapsed(&self) -> Duration {
            *self
                .elapsed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        fn sleeps(&self) -> Vec<Duration> {
            self.sleeps
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        }

        fn write_lock_held_during_sleep(&self) -> Vec<bool> {
            self.write_lock_held_during_sleep
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        }
    }

    #[async_trait]
    impl BarsIngestClock for FakeClock {
        fn instant(&self) -> Instant {
            self.started_at + self.elapsed()
        }

        fn utc_now(&self) -> DateTime<Utc> {
            self.started_at_utc + chrono::Duration::from_std(self.elapsed()).unwrap_or_default()
        }

        async fn sleep(&self, duration: Duration) {
            self.write_lock_held_during_sleep
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self.active_write_locks.load(Ordering::SeqCst) > 0);
            self.sleeps
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(duration);
            let mut elapsed = self
                .elapsed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *elapsed = elapsed.saturating_add(duration);
        }
    }

    #[async_trait]
    impl UsStockBarSource for FakeSource {
        async fn fetch_page(
            &self,
            query: &UsStockBarQuery,
            max_rows: Option<u32>,
            _deadline: Instant,
        ) -> Result<UsStockBarPage, UsStockBarSourceError> {
            self.calls.lock().await.push(query.clone());
            self.page_limits.lock().await.push(max_rows);
            let scripted_page = {
                let mut pages = self.scripted_pages.lock().await;
                (!pages.is_empty()).then(|| pages.remove(0))
            };
            if let Some(page) = scripted_page {
                return Ok(page);
            }
            let configured_bars = self.bars.lock().await.clone();
            let skipped_instruments = self.skipped_instruments.lock().await.clone();
            let bars = if configured_bars.is_empty() {
                query
                    .instrument_ids
                    .first()
                    .filter(|instrument_id| !skipped_instruments.contains_key(*instrument_id))
                    .map(|instrument_id| vec![bar(instrument_id, query.timeframe, query.to)])
                    .unwrap_or_default()
            } else {
                configured_bars
                    .into_iter()
                    .filter(|bar| {
                        query.instrument_ids.contains(&bar.instrument_id)
                            && !skipped_instruments.contains_key(&bar.instrument_id)
                            && bar.timeframe == query.timeframe
                            && bar.timestamp >= query.from
                            && bar.timestamp <= query.to
                    })
                    .collect()
            };
            let skipped = if query.page_token.is_none() {
                query
                    .instrument_ids
                    .iter()
                    .filter_map(|instrument_id| {
                        skipped_instruments.get(instrument_id).map(|reason| {
                            SkippedUsStockInstrument {
                                instrument_id: instrument_id.clone(),
                                reason: reason.clone(),
                            }
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Ok(UsStockBarPage {
                bars,
                next_page_token: None,
                skipped_instruments: skipped,
            })
        }

        async fn fetch_splits(
            &self,
            _query: &UsStockSplitQuery,
            _max_requests: usize,
            _deadline: Instant,
        ) -> Result<UsStockSplitBatch, UsStockBarSourceError> {
            Ok(UsStockSplitBatch::default())
        }
    }

    struct ImmediateWriteLock;

    struct ImmediateWriteLockGuard;

    struct BusyWriteLock;

    struct TrackingWriteLock {
        active_write_locks: Arc<AtomicUsize>,
    }

    struct TrackingWriteLockGuard {
        active_write_locks: Arc<AtomicUsize>,
    }

    impl UsStockBarsWriteLockGuard for ImmediateWriteLockGuard {}
    impl UsStockBarsWriteLockGuard for TrackingWriteLockGuard {}

    impl Drop for TrackingWriteLockGuard {
        fn drop(&mut self) {
            self.active_write_locks.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl UsStockBarsWriteLock for ImmediateWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: Duration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            Ok(Some(Box::new(ImmediateWriteLockGuard)))
        }
    }

    #[async_trait]
    impl UsStockBarsWriteLock for BusyWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: Duration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            Ok(None)
        }
    }

    #[async_trait]
    impl UsStockBarsWriteLock for TrackingWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: Duration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            self.active_write_locks.fetch_add(1, Ordering::SeqCst);
            Ok(Some(Box::new(TrackingWriteLockGuard {
                active_write_locks: self.active_write_locks.clone(),
            })))
        }
    }

    struct Harness {
        use_cases: UsStockBarsIngestUseCases,
        repository: Arc<crate::bars::FakeBarsRepository>,
        source: FakeSource,
        clock: Arc<FakeClock>,
    }

    fn harness() -> Harness {
        harness_with_lease(SharedSipRefetchLease::new(Arc::new(ImmediateWriteLock)))
    }

    fn harness_with_lease(lease: SharedSipRefetchLease) -> Harness {
        let repository = Arc::new(crate::bars::FakeBarsRepository::new());
        let repository_trait: SharedBarsRepository = repository.clone();
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let unit_of_work_trait: SharedUnitOfWork = unit_of_work;
        let active_write_locks = Arc::new(AtomicUsize::new(0));
        let clock = Arc::new(FakeClock::new(active_write_locks.clone()));
        let write_lock: SharedUsStockBarsWriteLock =
            Arc::new(TrackingWriteLock { active_write_locks });
        Harness {
            use_cases: UsStockBarsIngestUseCases::with_clock(
                BarsUseCases::new(unit_of_work_trait, repository_trait),
                write_lock,
                lease,
                clock.clone(),
            ),
            repository,
            source: FakeSource::default(),
            clock,
        }
    }

    fn without_runtime_waits(mut stats: UsStockBarsIngestStats) -> UsStockBarsIngestStats {
        stats.write_lock_wait_ms = 0;
        stats.write_throttle_wait_ms = 0;
        stats
    }

    fn bar(instrument_id: &str, timeframe: Timeframe, timestamp: chrono::DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: Decimal::new(101, 1),
            high: Decimal::new(111, 1),
            low: Decimal::new(91, 1),
            close: Decimal::new(105, 1),
            volume: 25,
            adjustment_factor: Decimal::ONE,
        }
    }

    fn fixed_now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 18, 16, 30, 0)
            .single()
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn skips_source_requests_when_the_sip_lease_is_busy() {
        let harness = harness_with_lease(SharedSipRefetchLease::new(Arc::new(BusyWriteLock)));
        let result = harness
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &harness.source,
                fixed_now(),
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10,
                },
            )
            .await
            .map(|result| without_runtime_waits(result.stats))
            .map_err(|error| error.to_string());
        let source_requests = harness.source.calls.lock().await.clone();

        assert_eq!(
            (result, source_requests),
            (
                Ok::<_, String>(UsStockBarsIngestStats {
                    interrupted_reason: Some("sip_refetch_lease_busy".to_owned()),
                    ..UsStockBarsIngestStats::default()
                }),
                Vec::<UsStockBarQuery>::new(),
            ),
        );
    }

    #[tokio::test]
    async fn rejects_page_that_would_exceed_the_sip_run_row_limit() {
        let harness = harness();
        let instrument_id = "DEMO-STOCK";
        let range = UsStockBarRange {
            from: fixed_now(),
            to: fixed_now() + chrono::Duration::days(1),
        };
        let page_bars = (0..1_000)
            .map(|index| {
                bar(
                    instrument_id,
                    Timeframe::Minute,
                    range.from + chrono::Duration::seconds(i64::from(index)),
                )
            })
            .collect::<Vec<_>>();
        harness
            .source
            .scripted_pages
            .lock()
            .await
            .push(UsStockBarPage {
                bars: page_bars,
                next_page_token: Some("page-next".to_owned()),
                skipped_instruments: Vec::new(),
            });
        let cursor = UsStockSipBarRefetchCursor {
            timeframe: UsStockSipBarRefetchTimeframe::Minute,
            range_from: range.from,
            range_to: range.to,
            page_token: Some("page-current".to_owned()),
        };
        harness
            .repository
            .us_stock_sip_refetch_cursors
            .lock()
            .await
            .insert(instrument_id.to_owned(), cursor.clone());
        let mut context = IngestContext {
            budget: RequestBudget::new(10, Duration::from_secs(60 * 60), harness.clock.instant()),
            stats: UsStockBarsIngestStats {
                daily_bars_upserted: 999_500,
                ..UsStockBarsIngestStats::default()
            },
            write_throttle: WriteThrottle::default(),
            sip_refetch: Some(SipRefetchLimits {
                max_rows_per_hour: 1_000_000,
                max_rows_per_run: SIP_REFETCH_MAX_ROWS_PER_RUN,
            }),
        };
        let outcome = {
            let mut batch_context = BatchIngestContext::new(&mut context);
            batch_context.sip_refetch_progress = Some(SipRefetchProgressContext {
                instrument_id: instrument_id.to_owned(),
                resume_after: Some(cursor.clone()),
            });
            batch_context.page_token = cursor.page_token.clone();
            harness
                .use_cases
                .ingest_batch(
                    &harness.source,
                    &[instrument_id.to_owned()],
                    UsStockBarTimeframe::Minute,
                    range,
                    &mut batch_context,
                )
                .await
                .map(|outcome| (outcome.completed, outcome.requested))
                .map_err(|error| error.to_string())
        };
        let reserved_rows = *harness
            .repository
            .us_stock_sip_refetch_rows_reserved
            .lock()
            .await;
        let saved_cursors = harness
            .repository
            .us_stock_sip_refetch_cursors
            .lock()
            .await
            .clone();
        let stored_bars = harness.repository.bars.lock().await.clone();
        let source_requests = harness.source.calls.lock().await.clone();
        let page_limits = harness.source.page_limits.lock().await.clone();
        let stats = without_runtime_waits(context.stats);

        assert_eq!(
            (
                outcome,
                stats,
                reserved_rows,
                saved_cursors,
                stored_bars,
                source_requests,
                page_limits,
            ),
            (
                Ok::<_, String>((false, true)),
                UsStockBarsIngestStats {
                    daily_bars_upserted: 999_500,
                    requests_attempted: 1,
                    interrupted_reason: Some("sip_refetch_run_row_limit_reached".to_owned()),
                    ..UsStockBarsIngestStats::default()
                },
                0,
                HashMap::from([(instrument_id.to_owned(), cursor)]),
                Vec::new(),
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Minute,
                    from: range.from,
                    to: range.to,
                    page_token: Some("page-current".to_owned()),
                }],
                vec![Some(1_000)],
            ),
        );
    }

    #[rstest]
    #[case::at_the_scheduled_minute(50, 0, 55 * 60)]
    #[case::after_the_scheduled_minute(55, 0, 50 * 60)]
    #[case::inside_the_safety_buffer(46, 0, 0)]
    #[case::before_the_scheduled_minute(20, 0, 25 * 60)]
    fn ends_before_the_next_sip_start(
        #[case] minute: u32,
        #[case] second: u32,
        #[case] expected_seconds: u64,
    ) {
        let now = Utc
            .with_ymd_and_hms(2026, 10, 18, 16, minute, second)
            .single()
            .unwrap_or_default();

        assert_eq!(
            super::run_duration_until_next_start(now),
            Duration::from_secs(expected_seconds),
        );
    }

    #[tokio::test]
    async fn refetches_existing_timeframes_with_the_required_ranges() {
        let fixture = harness();
        let instrument_id = "US:DEMO-ALPHA";
        let now = fixed_now();
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        fixture
            .repository
            .seed_bars(vec![
                bar(
                    instrument_id,
                    Timeframe::Daily,
                    now - chrono::Duration::days(5),
                ),
                bar(
                    instrument_id,
                    Timeframe::Hourly,
                    now - chrono::Duration::days(500),
                ),
                bar(
                    instrument_id,
                    Timeframe::Minute,
                    now - chrono::Duration::days(5),
                ),
            ])
            .await;

        let result = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10,
                },
            )
            .await
            .expect("SIP refetch succeeds");
        let cutoff = us_stock_sip_bars_history_cutoff(now);
        let hourly_cutoff = super::super::super::us_stock_hourly_history_cutoff(now);
        let queries = fixture.source.calls.lock().await.clone();
        let page_limits = fixture.source.page_limits.lock().await.clone();
        let pacing_millis = fixture
            .repository
            .us_stock_sip_refetch_pacing_millis
            .lock()
            .await
            .clone();
        let completed = fixture
            .repository
            .us_stock_sip_refetch_completed
            .lock()
            .await
            .clone();
        let hourly_progress = fixture
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .clone();

        assert_eq!(
            (
                without_runtime_waits(result.stats),
                queries,
                page_limits,
                pacing_millis,
                fixture.clock.sleeps(),
                fixture.clock.write_lock_held_during_sleep(),
                completed,
                hourly_progress,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 3,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 1,
                    hourly_bars_upserted: 1,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: Utc.timestamp_opt(0, 0).single().unwrap_or_default(),
                        to: cutoff,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Hourly,
                        from: Utc
                            .with_ymd_and_hms(2016, 1, 1, 0, 0, 0)
                            .single()
                            .unwrap_or_default(),
                        to: hourly_cutoff.min(cutoff),
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Minute,
                        from: hourly_cutoff,
                        to: cutoff,
                        page_token: None,
                    },
                ],
                vec![Some(1_000), Some(1_000), Some(1_000)],
                vec![3, 3, 3],
                vec![Duration::from_millis(3), Duration::from_millis(3)],
                vec![false, false],
                HashSet::from([instrument_id.to_owned()]),
                std::collections::HashMap::from([(
                    instrument_id.to_owned(),
                    hourly_cutoff.min(cutoff)
                )]),
            ),
        );
    }

    #[tokio::test]
    async fn leaves_a_symbol_pending_when_a_page_would_exceed_the_hourly_limit() {
        let fixture = harness();
        let instrument_id = "US:DEMO-BETA";
        let now = fixed_now();
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        fixture
            .repository
            .seed_bars(vec![bar(
                instrument_id,
                Timeframe::Daily,
                now - chrono::Duration::days(5),
            )])
            .await;
        *fixture.source.bars.lock().await = vec![
            bar(
                instrument_id,
                Timeframe::Daily,
                now - chrono::Duration::days(2),
            ),
            bar(
                instrument_id,
                Timeframe::Daily,
                now - chrono::Duration::days(1),
            ),
        ];

        let result = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 1,
                },
            )
            .await
            .expect("a rate-limited run returns its progress");
        let completed = fixture
            .repository
            .us_stock_sip_refetch_completed
            .lock()
            .await
            .clone();
        let reserved_rows = *fixture
            .repository
            .us_stock_sip_refetch_rows_reserved
            .lock()
            .await;

        assert_eq!(
            (
                result.stats,
                fixture.source.calls.lock().await.len(),
                completed,
                reserved_rows,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: Some("hourly_row_limit_reached".to_owned()),
                    compression_error: None,
                },
                1,
                HashSet::new(),
                0,
            ),
        );
    }

    #[tokio::test]
    async fn resumes_from_the_first_unwritten_page_after_reaching_the_hourly_limit() {
        let fixture = harness();
        let instrument_id = "US:DEMO-GAMMA";
        let now = fixed_now();
        fixture
            .repository
            .seed_us_stock_minute_targets(vec![instrument_id.to_owned()])
            .await;
        fixture
            .repository
            .seed_bars(vec![bar(
                instrument_id,
                Timeframe::Minute,
                now - chrono::Duration::days(5),
            )])
            .await;
        *fixture.source.scripted_pages.lock().await = vec![
            UsStockBarPage {
                bars: vec![bar(
                    instrument_id,
                    Timeframe::Minute,
                    now - chrono::Duration::days(4),
                )],
                next_page_token: Some("synthetic-page-two".to_owned()),
                skipped_instruments: Vec::new(),
            },
            UsStockBarPage {
                bars: vec![bar(
                    instrument_id,
                    Timeframe::Minute,
                    now - chrono::Duration::days(3),
                )],
                next_page_token: None,
                skipped_instruments: Vec::new(),
            },
        ];

        let first = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 1,
                },
            )
            .await
            .expect("the first page is committed before the hourly limit");
        let cursor_after_first = fixture
            .repository
            .us_stock_sip_refetch_cursors
            .lock()
            .await
            .clone();
        let second = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now + chrono::Duration::hours(1),
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 2,
                },
            )
            .await
            .expect("the next run resumes from the saved page token");
        let queries = fixture.source.calls.lock().await.clone();
        let completed = fixture
            .repository
            .us_stock_sip_refetch_completed
            .lock()
            .await
            .clone();

        let minute_range_start = super::super::super::us_stock_hourly_history_cutoff(now);
        let minute_range_end = us_stock_sip_bars_history_cutoff(now);
        let query = |page_token| UsStockBarQuery {
            instrument_ids: vec![instrument_id.to_owned()],
            timeframe: Timeframe::Minute,
            from: minute_range_start,
            to: minute_range_end,
            page_token,
        };
        assert_eq!(
            (
                without_runtime_waits(first.stats),
                cursor_after_first,
                without_runtime_waits(second.stats),
                queries,
                completed,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 2,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 1,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: Some("hourly_row_limit_reached".to_owned()),
                    compression_error: None,
                },
                HashMap::from([(
                    instrument_id.to_owned(),
                    UsStockSipBarRefetchCursor {
                        timeframe: UsStockSipBarRefetchTimeframe::Minute,
                        range_from: minute_range_start,
                        range_to: minute_range_end,
                        page_token: Some("synthetic-page-two".to_owned()),
                    },
                )]),
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 1,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
                },
                vec![
                    query(None),
                    query(Some("synthetic-page-two".to_owned())),
                    query(Some("synthetic-page-two".to_owned())),
                ],
                HashSet::from([instrument_id.to_owned()]),
            ),
        );
    }

    #[tokio::test]
    async fn records_a_skipped_symbol_and_does_not_request_it_again() {
        let fixture = harness();
        let instrument_id = "US:DEMO-DELTA";
        let reason = "synthetic instrument rejection";
        let now = fixed_now();
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        fixture
            .repository
            .seed_bars(vec![bar(
                instrument_id,
                Timeframe::Daily,
                now - chrono::Duration::days(5),
            )])
            .await;
        fixture
            .source
            .skipped_instruments
            .lock()
            .await
            .insert(instrument_id.to_owned(), reason.to_owned());

        let first = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10,
                },
            )
            .await
            .expect("the rejected instrument is recorded");
        let skipped = fixture
            .repository
            .us_stock_sip_refetch_skipped
            .lock()
            .await
            .clone();
        let second = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10,
                },
            )
            .await
            .expect("a skipped instrument is excluded from the next run");

        assert_eq!(
            (
                first.stats,
                skipped,
                second.stats,
                fixture.source.calls.lock().await.clone(),
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 0,
                    skipped_instruments: vec![SkippedUsStockInstrument {
                        instrument_id: instrument_id.to_owned(),
                        reason: reason.to_owned(),
                    }],
                    interrupted_reason: None,
                    compression_error: None,
                },
                HashMap::from([(instrument_id.to_owned(), reason.to_owned())]),
                UsStockBarsIngestStats::default(),
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Daily,
                    from: Utc.timestamp_opt(0, 0).single().unwrap_or_default(),
                    to: us_stock_sip_bars_history_cutoff(now),
                    page_token: None,
                }],
            ),
        );
    }

    #[tokio::test]
    async fn resumes_with_the_next_unfinished_symbol_and_stops_when_all_are_complete() {
        let fixture = harness();
        let first_instrument_id = "US:DEMO-ALPHA";
        let second_instrument_id = "US:DEMO-BETA";
        let now = fixed_now();
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![
                second_instrument_id.to_owned(),
                first_instrument_id.to_owned(),
            ])
            .await;
        fixture
            .repository
            .seed_bars(vec![
                bar(
                    first_instrument_id,
                    Timeframe::Daily,
                    now - chrono::Duration::days(5),
                ),
                bar(
                    second_instrument_id,
                    Timeframe::Daily,
                    now - chrono::Duration::days(5),
                ),
            ])
            .await;
        let config = UsStockSipBarRefetchConfig {
            max_symbols: Some(1),
            max_rows_per_hour: 10,
        };

        let first = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(&fixture.source, now, config)
            .await
            .expect("first symbol refetch succeeds");
        let second = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(&fixture.source, now, config)
            .await
            .expect("second symbol refetch succeeds");
        let final_run = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(&fixture.source, now, config)
            .await
            .expect("completed refetch is a no-op");
        let completed = fixture
            .repository
            .us_stock_sip_refetch_completed
            .lock()
            .await
            .clone();
        let requested_instruments = fixture
            .source
            .calls
            .lock()
            .await
            .iter()
            .map(|query| query.instrument_ids[0].clone())
            .collect::<Vec<_>>();

        assert_eq!(
            (
                vec![first.stats, second.stats, final_run.stats],
                requested_instruments,
                completed,
            ),
            (
                vec![
                    UsStockBarsIngestStats {
                        symbols_attempted: 1,
                        requests_attempted: 1,
                        daily_bars_upserted: 1,
                        minute_bars_upserted: 0,
                        hourly_bars_upserted: 0,
                        write_lock_wait_ms: 0,
                        write_throttle_wait_ms: 0,
                        skipped_instruments: Vec::new(),
                        interrupted_reason: None,
                        compression_error: None,
                    },
                    UsStockBarsIngestStats {
                        symbols_attempted: 1,
                        requests_attempted: 1,
                        daily_bars_upserted: 1,
                        minute_bars_upserted: 0,
                        hourly_bars_upserted: 0,
                        write_lock_wait_ms: 0,
                        write_throttle_wait_ms: 3,
                        skipped_instruments: Vec::new(),
                        interrupted_reason: None,
                        compression_error: None,
                    },
                    UsStockBarsIngestStats::default(),
                ],
                vec![
                    first_instrument_id.to_owned(),
                    second_instrument_id.to_owned()
                ],
                HashSet::from([
                    first_instrument_id.to_owned(),
                    second_instrument_id.to_owned(),
                ]),
            ),
        );
    }

    #[tokio::test]
    async fn paces_one_thousand_rows_for_three_seconds() {
        let fixture = harness();
        let instrument_id = "US:DEMO-SIP-BULK";
        let now = fixed_now();
        let bars = (1..=1_000_i64)
            .map(|days_ago| {
                bar(
                    instrument_id,
                    Timeframe::Daily,
                    now - chrono::Duration::days(days_ago),
                )
            })
            .collect::<Vec<_>>();
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        fixture.repository.seed_bars(bars.clone()).await;
        *fixture.source.bars.lock().await = bars;

        let result = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10_000,
                },
            )
            .await
            .expect("SIP refetch succeeds");
        let actual = (
            result.stats,
            fixture.source.page_limits.lock().await.clone(),
            fixture
                .repository
                .us_stock_sip_refetch_pacing_millis
                .lock()
                .await
                .clone(),
        );

        assert_eq!(
            actual,
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: 1_000,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
                },
                vec![Some(1_000)],
                vec![3_000],
            ),
        );
    }

    #[tokio::test]
    async fn waits_for_persisted_pacing_before_the_first_page_after_restart() {
        let fixture = harness();
        let instrument_id = "US:DEMO-SIP-RESTART";
        let now = fixed_now();
        let stored_deadline = fixture.clock.utc_now() + chrono::Duration::seconds(5);
        *fixture
            .repository
            .us_stock_sip_refetch_next_allowed_at
            .lock()
            .await = Some(stored_deadline);
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        let existing_bar = bar(
            instrument_id,
            Timeframe::Daily,
            now - chrono::Duration::days(5),
        );
        fixture
            .repository
            .seed_bars(vec![existing_bar.clone()])
            .await;
        *fixture.source.bars.lock().await = vec![existing_bar];

        let result = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10,
                },
            )
            .await
            .expect("SIP refetch succeeds");
        let actual = (
            result.stats,
            fixture.clock.sleeps(),
            fixture.clock.write_lock_held_during_sleep(),
            fixture.source.page_limits.lock().await.clone(),
        );

        assert_eq!(
            actual,
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 5_000,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
                },
                vec![Duration::from_secs(5)],
                vec![false],
                vec![Some(1_000)],
            ),
        );
    }

    #[tokio::test]
    async fn interrupts_before_the_deadline_and_keeps_the_current_page_cursor() {
        let fixture = harness();
        let instrument_id = "US:DEMO-SIP-DEADLINE";
        let now = Utc
            .with_ymd_and_hms(2026, 10, 18, 16, 44, 55)
            .single()
            .unwrap_or_default();
        let cursor = UsStockSipBarRefetchCursor {
            timeframe: UsStockSipBarRefetchTimeframe::Daily,
            range_from: now - chrono::Duration::days(10),
            range_to: now - chrono::Duration::days(1),
            page_token: Some("synthetic-page-token".to_owned()),
        };
        fixture
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        fixture
            .repository
            .seed_bars(vec![bar(
                instrument_id,
                Timeframe::Daily,
                now - chrono::Duration::days(5),
            )])
            .await;
        fixture
            .repository
            .us_stock_sip_refetch_cursors
            .lock()
            .await
            .insert(instrument_id.to_owned(), cursor.clone());
        *fixture
            .repository
            .us_stock_sip_refetch_next_allowed_at
            .lock()
            .await = Some(fixture.clock.utc_now() + chrono::Duration::seconds(60));

        let result = fixture
            .use_cases
            .refetch_us_stock_bars_via_sip(
                &fixture.source,
                now,
                UsStockSipBarRefetchConfig {
                    max_symbols: None,
                    max_rows_per_hour: 10,
                },
            )
            .await
            .expect("deadline interruption is reported in stats");
        let actual = (
            result.stats,
            fixture.source.calls.lock().await.clone(),
            fixture.source.page_limits.lock().await.clone(),
            fixture.clock.sleeps(),
            fixture.clock.write_lock_held_during_sleep(),
            fixture
                .repository
                .us_stock_sip_refetch_cursors
                .lock()
                .await
                .clone(),
            fixture
                .repository
                .us_stock_sip_refetch_pacing_millis
                .lock()
                .await
                .clone(),
        );

        assert_eq!(
            actual,
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    write_lock_wait_ms: 0,
                    write_throttle_wait_ms: 5_000,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: Some("time_budget_exhausted".to_owned()),
                    compression_error: None,
                },
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Daily,
                    from: cursor.range_from,
                    to: cursor.range_to,
                    page_token: cursor.page_token.clone(),
                }],
                vec![Some(1_000)],
                vec![Duration::from_secs(5)],
                vec![false],
                HashMap::from([(instrument_id.to_owned(), cursor)]),
                Vec::<u64>::new(),
            ),
        );
    }
}
