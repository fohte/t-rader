use chrono::{DateTime, Duration, Utc};
use std::time::Duration as StdDuration;
use std::time::Instant;

use super::error::BarsUseCaseError;
use super::hourly_bar_bucket_start;
use super::types::{UsStockMinuteBarsDownsampleConfig, UsStockMinuteBarsDownsampleStats};
use super::us_stock_hourly_history_cutoff;
use super::use_cases::UsStockBarsIngestUseCases;
use super::write_lock::{UsStockBarsWriteLock, UsStockBarsWriteLockGuard};

const DOWNSAMPLE_BATCH_DAYS: i64 = 7;
const DOWNSAMPLE_REST_MULTIPLIER: u32 = 9;
const WRITE_LOCK_WAIT_TIMEOUT: StdDuration = StdDuration::from_secs(30);

async fn acquire_us_stock_bars_write_lock(
    write_lock: &dyn UsStockBarsWriteLock,
    wait_timeout: StdDuration,
) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, BarsUseCaseError> {
    match tokio::time::timeout(wait_timeout, write_lock.acquire(wait_timeout)).await {
        Ok(Ok(guard)) => Ok(guard),
        Err(_) => Ok(None),
        Ok(Err(error)) => Err(error.into()),
    }
}

fn processing_budget_exhausted(
    clock: &dyn super::clock::BarsIngestClock,
    started_at: Instant,
    paused_for: StdDuration,
    budget: Option<StdDuration>,
) -> bool {
    budget.is_some_and(|budget| {
        clock
            .instant()
            .saturating_duration_since(started_at)
            .saturating_sub(paused_for)
            >= budget
    })
}

impl UsStockBarsIngestUseCases {
    pub async fn downsample_old_us_stock_minute_bars(
        &self,
        now: DateTime<Utc>,
        config: UsStockMinuteBarsDownsampleConfig,
    ) -> Result<UsStockMinuteBarsDownsampleStats, BarsUseCaseError> {
        let processing_started_at = self.clock.instant();
        let mut paused_for = StdDuration::ZERO;
        let cutoff = us_stock_hourly_history_cutoff(now);
        let Some(oldest_bar) = self
            .bars
            .repository
            .find_earliest_us_stock_minute_bar_before(cutoff)
            .await?
        else {
            return Ok(UsStockMinuteBarsDownsampleStats::default());
        };

        let mut stats = UsStockMinuteBarsDownsampleStats::default();
        if processing_budget_exhausted(
            self.clock.as_ref(),
            processing_started_at,
            paused_for,
            config.max_processing_duration,
        ) {
            return Ok(stats);
        }

        let mut batch_from = hourly_bar_bucket_start(oldest_bar);
        while batch_from < cutoff {
            let batch_until = batch_from
                .checked_add_signed(Duration::days(DOWNSAMPLE_BATCH_DAYS))
                .unwrap_or(cutoff);
            let instrument_ids = self
                .bars
                .repository
                .find_us_stock_minute_bar_instrument_ids(batch_from, batch_until, cutoff)
                .await?;
            if processing_budget_exhausted(
                self.clock.as_ref(),
                processing_started_at,
                paused_for,
                config.max_processing_duration,
            ) {
                tracing::info!(
                    reason = "processing_budget_exhausted",
                    batch_from = %batch_from,
                    hourly_bars_inserted = stats.hourly_bars_inserted,
                    minute_bars_deleted = stats.minute_bars_deleted,
                    "米国株の分足間引きを次回の実行に延期しました"
                );
                return Ok(stats);
            }

            for instrument_id in &instrument_ids {
                if processing_budget_exhausted(
                    self.clock.as_ref(),
                    processing_started_at,
                    paused_for,
                    config.max_processing_duration,
                ) {
                    tracing::info!(
                        reason = "processing_budget_exhausted",
                        batch_from = %batch_from,
                        hourly_bars_inserted = stats.hourly_bars_inserted,
                        minute_bars_deleted = stats.minute_bars_deleted,
                        "米国株の分足間引きを次回の実行に延期しました"
                    );
                    return Ok(stats);
                }

                let Some(write_lock_guard) = acquire_us_stock_bars_write_lock(
                    self.us_stock_bars_write_lock.as_ref(),
                    WRITE_LOCK_WAIT_TIMEOUT,
                )
                .await?
                else {
                    tracing::info!(
                        reason = "write_lock_busy",
                        batch_from = %batch_from,
                        hourly_bars_inserted = stats.hourly_bars_inserted,
                        minute_bars_deleted = stats.minute_bars_deleted,
                        "米国株の分足間引きを次回の実行に延期しました"
                    );
                    return Ok(stats);
                };

                if processing_budget_exhausted(
                    self.clock.as_ref(),
                    processing_started_at,
                    paused_for,
                    config.max_processing_duration,
                ) {
                    drop(write_lock_guard);
                    tracing::info!(
                        reason = "processing_budget_exhausted",
                        batch_from = %batch_from,
                        hourly_bars_inserted = stats.hourly_bars_inserted,
                        minute_bars_deleted = stats.minute_bars_deleted,
                        "米国株の分足間引きを次回の実行に延期しました"
                    );
                    return Ok(stats);
                }

                let transaction_started_at = self.clock.instant();
                let batch_result = async {
                    let transaction = self.bars.unit_of_work.begin().await?;
                    let batch_stats = self
                        .bars
                        .repository
                        .downsample_us_stock_minute_bar_range(
                            &transaction,
                            instrument_id,
                            batch_from,
                            batch_until,
                            cutoff,
                        )
                        .await?;
                    self.bars.unit_of_work.commit(transaction).await?;
                    Ok::<_, BarsUseCaseError>(batch_stats)
                }
                .await;
                let transaction_duration = self
                    .clock
                    .instant()
                    .saturating_duration_since(transaction_started_at);
                drop(write_lock_guard);
                let batch_stats = match batch_result {
                    Ok(batch_stats) => batch_stats,
                    Err(error) => {
                        tracing::error!(
                            instrument_id = %instrument_id,
                            error = %error,
                            "米国株の分足を集計できませんでした"
                        );
                        return Err(error);
                    }
                };

                stats.hourly_bars_inserted += batch_stats.hourly_bars_inserted;
                stats.minute_bars_deleted += batch_stats.minute_bars_deleted;

                if processing_budget_exhausted(
                    self.clock.as_ref(),
                    processing_started_at,
                    paused_for,
                    config.max_processing_duration,
                ) {
                    tracing::info!(
                        reason = "processing_budget_exhausted",
                        batch_from = %batch_from,
                        hourly_bars_inserted = stats.hourly_bars_inserted,
                        minute_bars_deleted = stats.minute_bars_deleted,
                        "米国株の分足間引きを次回の実行に延期しました"
                    );
                    return Ok(stats);
                }

                if config.max_processing_duration.is_some() {
                    let rest = transaction_duration.saturating_mul(DOWNSAMPLE_REST_MULTIPLIER);
                    if !rest.is_zero() {
                        self.clock.sleep(rest).await;
                        paused_for = paused_for.saturating_add(rest);
                    }
                }
            }
            batch_from = batch_until;
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Instant;

    use chrono::{DateTime, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration as StdDuration;

    use crate::bars::{
        BarsUseCases, FakeBarsRepository, SharedUsStockBarsWriteLock, UsStockBarsIngestUseCases,
        UsStockBarsWriteLock, UsStockBarsWriteLockGuard, UsStockMinuteBarsDownsampleConfig,
        UsStockMinuteBarsDownsampleStats,
    };
    use crate::persistence::PersistenceError;

    use super::super::clock::BarsIngestClock;
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::acquire_us_stock_bars_write_lock;

    struct BlockingWriteLock;

    #[async_trait::async_trait]
    impl UsStockBarsWriteLock for BlockingWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: StdDuration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            std::future::pending().await
        }
    }

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .to_utc()
    }

    fn minute_bar(instrument_id: &str, timestamp: DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe: Timeframe::Minute,
            timestamp,
            open: Decimal::ONE,
            high: Decimal::ONE,
            low: Decimal::ONE,
            close: Decimal::ONE,
            volume: 1,
            adjustment_factor: Decimal::ONE,
        }
    }

    struct DownsampleFixture {
        use_cases: UsStockBarsIngestUseCases,
        unit_of_work: Arc<FakeUnitOfWork>,
        repository: Arc<FakeBarsRepository>,
        write_lock: Arc<TestDownsampleWriteLock>,
        clock: Arc<TestDownsampleClock>,
    }

    struct TestDownsampleWriteLock {
        busy_on_attempt: AtomicUsize,
        attempts: AtomicUsize,
        active: Arc<AtomicUsize>,
    }

    impl TestDownsampleWriteLock {
        fn new() -> Self {
            Self {
                busy_on_attempt: AtomicUsize::new(0),
                attempts: AtomicUsize::new(0),
                active: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn set_busy_on_attempt(&self, attempt: usize) {
            self.busy_on_attempt.store(attempt, Ordering::SeqCst);
        }

        fn attempts(&self) -> usize {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    struct TestDownsampleWriteLockGuard(Arc<AtomicUsize>);

    impl Drop for TestDownsampleWriteLockGuard {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl UsStockBarsWriteLockGuard for TestDownsampleWriteLockGuard {}

    struct TestDownsampleClock {
        now: StdMutex<Instant>,
        lock_active: Arc<AtomicUsize>,
        sleeps: StdMutex<Vec<StdDuration>>,
        lock_counts_during_sleep: StdMutex<Vec<usize>>,
    }

    impl TestDownsampleClock {
        fn new(lock_active: Arc<AtomicUsize>) -> Self {
            Self {
                now: StdMutex::new(Instant::now()),
                lock_active,
                sleeps: StdMutex::new(Vec::new()),
                lock_counts_during_sleep: StdMutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl BarsIngestClock for TestDownsampleClock {
        fn instant(&self) -> Instant {
            let mut now = self.now.lock().expect("clock lock is available");
            let current = *now;
            *now += StdDuration::from_secs(1);
            current
        }

        async fn sleep(&self, duration: StdDuration) {
            self.sleeps
                .lock()
                .expect("sleep history lock is available")
                .push(duration);
            self.lock_counts_during_sleep
                .lock()
                .expect("lock history lock is available")
                .push(self.lock_active.load(Ordering::SeqCst));
            *self.now.lock().expect("clock lock is available") += duration;
        }
    }

    #[async_trait::async_trait]
    impl UsStockBarsWriteLock for TestDownsampleWriteLock {
        async fn acquire(
            &self,
            wait_timeout: StdDuration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if wait_timeout.is_zero() || self.busy_on_attempt.load(Ordering::SeqCst) == attempt {
                return Ok(None);
            }

            self.active.fetch_add(1, Ordering::SeqCst);
            Ok(Some(Box::new(TestDownsampleWriteLockGuard(
                self.active.clone(),
            ))))
        }
    }

    #[fixture]
    async fn downsample_fixture() -> DownsampleFixture {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeBarsRepository::new());
        let write_lock = Arc::new(TestDownsampleWriteLock::new());
        let clock = Arc::new(TestDownsampleClock::new(write_lock.active.clone()));
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let bars_use_cases = BarsUseCases::new(shared_unit_of_work, repository.clone());
        let shared_write_lock: SharedUsStockBarsWriteLock = write_lock.clone();
        let use_cases =
            UsStockBarsIngestUseCases::with_clock(bars_use_cases, shared_write_lock, clock.clone());
        DownsampleFixture {
            use_cases,
            unit_of_work,
            repository,
            write_lock,
            clock,
        }
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_processes_aged_minutes_in_hour_aligned_weekly_transactions(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-21T03:05:00Z")),
                minute_bar("US:DEMO-ALPHA", date("2038-12-29T10:05:00Z")),
                minute_bar("US:DEMO-ALPHA", date("2039-01-01T12:05:00Z")),
                minute_bar("JP-DEMO", date("2038-12-20T03:05:00Z")),
            ])
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"), Default::default())
            .await;
        let downsample_instruments = repository
            .minute_bar_downsample_instruments
            .lock()
            .await
            .clone();
        let downsample_ranges = repository.minute_bar_downsample_ranges.lock().await.clone();
        let cutoffs = repository
            .minute_bar_downsample_cutoffs
            .lock()
            .await
            .clone();
        let write_transaction_ids = repository.write_transaction_ids.lock().await.clone();
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let write_lock_attempts = downsample_fixture.write_lock.attempts();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();

        assert_eq!(
            (
                result.map_err(|_| ()),
                downsample_instruments,
                downsample_ranges,
                cutoffs,
                begun.len(),
                write_lock_attempts,
                write_transaction_ids == begun,
                committed == begun,
            ),
            (
                Ok(Default::default()),
                vec![
                    "US:DEMO-ALPHA".to_owned(),
                    "US:DEMO-BETA".to_owned(),
                    "US:DEMO-ALPHA".to_owned(),
                ],
                vec![
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-27T03:00:00Z"), date("2039-01-03T03:00:00Z"),),
                ],
                vec![
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                ],
                3,
                3,
                true,
                true,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_failure_keeps_prior_instrument_commits(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-20T03:05:00Z")),
            ])
            .await;
        repository
            .fail_minute_bar_downsampling_for("US:DEMO-BETA", "injected database failure")
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"), Default::default())
            .await
            .map_err(|_| ());
        let downsample_instruments = repository
            .minute_bar_downsample_instruments
            .lock()
            .await
            .clone();
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();
        let write_transaction_ids = repository.write_transaction_ids.lock().await.clone();
        let write_lock_attempts = downsample_fixture.write_lock.attempts();
        let transaction_results = begun
            .iter()
            .map(|transaction_id| {
                (
                    write_transaction_ids.contains(transaction_id),
                    committed.contains(transaction_id),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            (
                result,
                downsample_instruments,
                transaction_results,
                write_lock_attempts,
            ),
            (
                Err(()),
                vec!["US:DEMO-ALPHA".to_owned(), "US:DEMO-BETA".to_owned()],
                vec![(true, true), (true, false)],
                2,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_returns_without_opening_transactions_when_write_lock_is_busy(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-20T03:05:00Z")),
            ])
            .await;
        downsample_fixture.write_lock.set_busy_on_attempt(1);

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"), Default::default())
            .await
            .map_err(|_| ());
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let downsample_ranges = repository.minute_bar_downsample_ranges.lock().await.clone();

        assert_eq!(
            (
                result,
                downsample_fixture.write_lock.attempts(),
                begun,
                downsample_ranges,
            ),
            (Ok(Default::default()), 1, Vec::new(), Vec::new()),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_returns_completed_batch_stats_when_write_lock_becomes_busy(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-20T03:05:00Z")),
            ])
            .await;
        repository
            .set_minute_bar_downsample_stats(UsStockMinuteBarsDownsampleStats {
                hourly_bars_inserted: 1,
                minute_bars_deleted: 3,
            })
            .await;
        downsample_fixture.write_lock.set_busy_on_attempt(2);

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"), Default::default())
            .await
            .map_err(|_| ());
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();
        let downsample_ranges = repository.minute_bar_downsample_ranges.lock().await.clone();

        assert_eq!(
            (
                result,
                downsample_fixture.write_lock.attempts(),
                begun.len(),
                committed == begun,
                downsample_ranges,
            ),
            (
                Ok(UsStockMinuteBarsDownsampleStats {
                    hourly_bars_inserted: 1,
                    minute_bars_deleted: 3,
                }),
                2,
                1,
                true,
                vec![(date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_budget_includes_candidate_lookup_before_starting_transactions(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![minute_bar(
                "US:DEMO-ALPHA",
                date("2038-12-30T03:05:00Z"),
            )])
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(
                date("2040-01-01T12:00:00Z"),
                UsStockMinuteBarsDownsampleConfig {
                    max_processing_duration: Some(StdDuration::from_secs(2)),
                },
            )
            .await
            .map_err(|_| ());
        let candidate_ranges = repository
            .minute_bar_downsample_candidate_ranges
            .lock()
            .await
            .clone();
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();

        assert_eq!(
            (
                result,
                candidate_ranges,
                begun,
                downsample_fixture.write_lock.attempts(),
            ),
            (
                Ok(Default::default()),
                vec![(date("2038-12-30T03:00:00Z"), date("2039-01-06T03:00:00Z"))],
                Vec::new(),
                0,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_budget_keeps_committed_transactions_and_stops_before_the_next(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-30T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-30T03:05:00Z")),
            ])
            .await;
        repository
            .set_minute_bar_downsample_stats(UsStockMinuteBarsDownsampleStats {
                hourly_bars_inserted: 1,
                minute_bars_deleted: 3,
            })
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(
                date("2040-01-01T12:00:00Z"),
                UsStockMinuteBarsDownsampleConfig {
                    max_processing_duration: Some(StdDuration::from_secs(7)),
                },
            )
            .await
            .map_err(|_| ());
        let downsample_instruments = repository
            .minute_bar_downsample_instruments
            .lock()
            .await
            .clone();
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();

        assert_eq!(
            (
                result,
                downsample_instruments,
                begun.len(),
                committed == begun,
                downsample_fixture.write_lock.attempts(),
                downsample_fixture
                    .clock
                    .sleeps
                    .lock()
                    .expect("sleep history lock is available")
                    .clone(),
            ),
            (
                Ok(UsStockMinuteBarsDownsampleStats {
                    hourly_bars_inserted: 1,
                    minute_bars_deleted: 3,
                }),
                vec!["US:DEMO-ALPHA".to_owned()],
                1,
                true,
                1,
                Vec::new(),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    #[awt]
    async fn downsample_rests_nine_transaction_durations_after_releasing_the_write_lock(
        #[future] downsample_fixture: DownsampleFixture,
    ) {
        let repository = &downsample_fixture.repository;
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-30T03:05:00Z")),
                minute_bar("US:DEMO-BETA", date("2038-12-30T03:05:00Z")),
            ])
            .await;

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(
                date("2040-01-01T12:00:00Z"),
                UsStockMinuteBarsDownsampleConfig {
                    max_processing_duration: Some(StdDuration::from_secs(60)),
                },
            )
            .await
            .map_err(|_| ());
        let begun = downsample_fixture.unit_of_work.begun.lock().await.clone();
        let committed = downsample_fixture
            .unit_of_work
            .committed
            .lock()
            .await
            .clone();

        assert_eq!(
            (
                result,
                begun.len(),
                committed == begun,
                downsample_fixture.write_lock.attempts(),
                downsample_fixture
                    .clock
                    .sleeps
                    .lock()
                    .expect("sleep history lock is available")
                    .clone(),
                downsample_fixture
                    .clock
                    .lock_counts_during_sleep
                    .lock()
                    .expect("lock history lock is available")
                    .clone(),
            ),
            (
                Ok(Default::default()),
                2,
                true,
                2,
                vec![StdDuration::from_secs(9), StdDuration::from_secs(9)],
                vec![0, 0],
            ),
        );
    }

    #[tokio::test]
    async fn downsample_write_lock_wait_is_bounded() {
        let result =
            acquire_us_stock_bars_write_lock(&BlockingWriteLock, StdDuration::from_millis(1))
                .await
                .map(|guard| guard.is_some())
                .map_err(|_| "other");

        assert_eq!(result, Ok(false));
    }
}
