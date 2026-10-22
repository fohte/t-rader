use chrono::{DateTime, Duration, Utc};
use std::time::Duration as StdDuration;

use super::error::BarsUseCaseError;
use super::hourly_bar_bucket_start;
use super::types::UsStockMinuteBarsDownsampleStats;
use super::us_stock_hourly_history_cutoff;
use super::use_cases::UsStockBarsIngestUseCases;
use super::write_lock::{UsStockBarsWriteLock, UsStockBarsWriteLockGuard};

const DOWNSAMPLE_BATCH_DAYS: i64 = 7;
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

impl UsStockBarsIngestUseCases {
    pub async fn downsample_old_us_stock_minute_bars(
        &self,
        now: DateTime<Utc>,
    ) -> Result<UsStockMinuteBarsDownsampleStats, BarsUseCaseError> {
        let cutoff = us_stock_hourly_history_cutoff(now);
        let Some(oldest_bar) = self
            .bars
            .repository
            .find_earliest_us_stock_minute_bar_before(cutoff)
            .await?
        else {
            return Ok(UsStockMinuteBarsDownsampleStats::default());
        };

        let mut batch_from = hourly_bar_bucket_start(oldest_bar);
        let mut stats = UsStockMinuteBarsDownsampleStats::default();
        let instrument_ids = self.bars.repository.find_us_stock_instrument_ids().await?;
        while batch_from < cutoff {
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

            let batch_until = batch_from
                .checked_add_signed(Duration::days(DOWNSAMPLE_BATCH_DAYS))
                .unwrap_or(cutoff);
            for instrument_id in &instrument_ids {
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
            }
            batch_from = batch_until;
            drop(write_lock_guard);
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration as StdDuration;

    use crate::bars::{
        BarsUseCases, FakeBarsRepository, SharedSipRefetchLease, SharedUsStockBarsWriteLock,
        UsStockBarsIngestUseCases, UsStockBarsWriteLock, UsStockBarsWriteLockGuard,
        UsStockMinuteBarsDownsampleStats,
    };
    use crate::persistence::PersistenceError;

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
    }

    struct TestDownsampleWriteLock {
        busy_on_attempt: AtomicUsize,
        attempts: AtomicUsize,
    }

    impl TestDownsampleWriteLock {
        fn new() -> Self {
            Self {
                busy_on_attempt: AtomicUsize::new(0),
                attempts: AtomicUsize::new(0),
            }
        }

        fn set_busy_on_attempt(&self, attempt: usize) {
            self.busy_on_attempt.store(attempt, Ordering::SeqCst);
        }

        fn attempts(&self) -> usize {
            self.attempts.load(Ordering::SeqCst)
        }
    }

    struct TestDownsampleWriteLockGuard;

    impl UsStockBarsWriteLockGuard for TestDownsampleWriteLockGuard {}

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

            Ok(Some(Box::new(TestDownsampleWriteLockGuard)))
        }
    }

    #[fixture]
    async fn downsample_fixture() -> DownsampleFixture {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeBarsRepository::new());
        let write_lock = Arc::new(TestDownsampleWriteLock::new());
        repository
            .seed_us_stock_instrument_ids(vec![
                "US:DEMO-ALPHA".to_owned(),
                "US:DEMO-BETA".to_owned(),
            ])
            .await;
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let bars_use_cases = BarsUseCases::new(shared_unit_of_work, repository.clone());
        let shared_write_lock: SharedUsStockBarsWriteLock = write_lock.clone();
        let use_cases = UsStockBarsIngestUseCases::new(
            bars_use_cases,
            shared_write_lock.clone(),
            SharedSipRefetchLease::new(shared_write_lock),
        );
        DownsampleFixture {
            use_cases,
            unit_of_work,
            repository,
            write_lock,
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
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
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
                    "US:DEMO-BETA".to_owned(),
                ],
                vec![
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-27T03:00:00Z"), date("2039-01-03T03:00:00Z"),),
                    (date("2038-12-27T03:00:00Z"), date("2039-01-03T03:00:00Z"),),
                ],
                vec![
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                    date("2039-01-01T12:00:00Z"),
                ],
                4,
                2,
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
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
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
                1,
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
            .seed_bars(vec![minute_bar(
                "US:DEMO-ALPHA",
                date("2038-12-20T03:05:00Z"),
            )])
            .await;
        downsample_fixture.write_lock.set_busy_on_attempt(1);

        let result = downsample_fixture
            .use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
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
            .seed_bars(vec![minute_bar(
                "US:DEMO-ALPHA",
                date("2038-12-20T03:05:00Z"),
            )])
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
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
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
                    hourly_bars_inserted: 2,
                    minute_bars_deleted: 6,
                }),
                2,
                2,
                true,
                vec![
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                ],
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
