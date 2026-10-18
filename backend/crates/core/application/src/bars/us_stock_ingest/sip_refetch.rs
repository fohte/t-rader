use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};

use super::super::error::{BarsUseCaseError, UsStockBarsIngestError};
use super::super::types::{
    UsStockBarTarget, UsStockBarsIngestResult, UsStockBarsIngestStats, UsStockSipBarRefetchConfig,
};
use super::super::us_stock_source::UsStockBarSource;
use super::super::use_cases::UsStockBarsIngestUseCases;
use super::super::write_throttle::WriteThrottle;
use super::request_budget::RequestBudget;
use super::{BatchIngestContext, IngestContext, UsStockBarRange, UsStockBarTimeframe};

const MAX_REQUESTS_PER_RUN: usize = usize::MAX;
const MAX_RUN_DURATION: Duration = Duration::from_secs(90 * 60);
const HOURLY_HISTORY_START: (i32, u32, u32) = (2016, 1, 1);

impl UsStockBarsIngestUseCases {
    pub async fn refetch_us_stock_bars_via_sip(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
        config: UsStockSipBarRefetchConfig,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        let mut budget =
            RequestBudget::new(MAX_REQUESTS_PER_RUN, MAX_RUN_DURATION, self.clock.instant());
        let mut lock_wait_stats = UsStockBarsIngestStats::default();
        let write_lock_guard = match self
            .acquire_us_stock_bars_write_lock(&mut budget, &mut lock_wait_stats)
            .await
        {
            Ok(Some(guard)) => guard,
            Ok(None) => return Ok(UsStockBarsIngestResult::new(lock_wait_stats, None)),
            Err(error) => {
                return Err(UsStockBarsIngestError::new(
                    UsStockBarsIngestResult::new(lock_wait_stats, None),
                    error,
                ));
            }
        };

        let mut context = IngestContext {
            budget,
            stats: UsStockBarsIngestStats::default(),
            write_throttle: WriteThrottle::default(),
            max_rows_per_hour: Some(config.max_rows_per_hour),
        };
        context.stats.write_lock_wait_ms = lock_wait_stats.write_lock_wait_ms;
        let refetch_result = self
            .refetch_pending_targets(source, now, config, &mut context)
            .await;
        match refetch_result {
            Ok(completed) => {
                if !completed && context.stats.interrupted_reason.is_none() {
                    context.stats.interrupted_reason =
                        context.budget.interrupted_reason.map(str::to_owned);
                }
                Ok(UsStockBarsIngestResult::new(
                    context.stats,
                    Some(write_lock_guard),
                ))
            }
            Err(error) => Err(UsStockBarsIngestError::new(
                UsStockBarsIngestResult::new(context.stats, Some(write_lock_guard)),
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

        let daily_range = UsStockBarRange {
            from: UsStockBarTimeframe::Daily.history_start(now),
            to: UsStockBarTimeframe::Daily.history_end(now),
        };
        let hourly_range = UsStockBarRange {
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
        };
        let minute_range = UsStockBarRange {
            from: UsStockBarTimeframe::Minute.history_start(now),
            to: UsStockBarTimeframe::Minute.history_end(now),
        };

        for target in targets {
            if !self
                .refetch_target(
                    source,
                    &target,
                    daily_range,
                    hourly_range,
                    minute_range,
                    context,
                )
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
        target: &UsStockBarTarget,
        daily_range: UsStockBarRange,
        hourly_range: UsStockBarRange,
        minute_range: UsStockBarRange,
        context: &mut IngestContext,
    ) -> Result<bool, BarsUseCaseError> {
        let instrument_ids = std::slice::from_ref(&target.instrument_id);
        let frames = [
            (
                UsStockBarTimeframe::Daily,
                target.earliest_daily_bar,
                daily_range,
            ),
            (
                UsStockBarTimeframe::Hourly,
                target.earliest_hourly_bar,
                hourly_range,
            ),
            (
                UsStockBarTimeframe::Minute,
                target.earliest_minute_bar,
                minute_range,
            ),
        ];

        for (timeframe, earliest_bar, range) in frames {
            if earliest_bar.is_none() {
                continue;
            }
            let frame_result = self
                .ingest_refetch_frame(source, instrument_ids, timeframe, range, context)
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
            if timeframe == UsStockBarTimeframe::Hourly {
                self.mark_hourly_history_completed(instrument_ids, range.to, &Default::default())
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
        instrument_ids: &[String],
        timeframe: UsStockBarTimeframe,
        range: UsStockBarRange,
        context: &mut IngestContext,
    ) -> Result<RefetchFrameResult, BarsUseCaseError> {
        let (completed, skipped_instrument_ids) = {
            let mut batch_context = BatchIngestContext::new(context);
            let completed = self
                .ingest_batch(source, instrument_ids, timeframe, range, &mut batch_context)
                .await?;
            (completed, batch_context.skipped_instrument_ids)
        };
        if !completed {
            return Ok(RefetchFrameResult::Interrupted);
        }

        let Some(instrument_id) = instrument_ids.first() else {
            return Ok(RefetchFrameResult::Completed);
        };
        if !skipped_instrument_ids.contains(instrument_id) {
            return Ok(RefetchFrameResult::Completed);
        }
        let reason = context
            .stats
            .skipped_instruments
            .iter()
            .find(|skipped| skipped.instrument_id == *instrument_id)
            .map(|skipped| skipped.reason.clone())
            .unwrap_or_else(|| "Alpaca rejected the instrument".to_owned());
        Ok(RefetchFrameResult::Skipped(reason))
    }
}

enum RefetchFrameResult {
    Completed,
    Interrupted,
    Skipped(String),
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::{
        collections::HashSet,
        sync::Arc,
        time::{Duration, Instant},
    };

    use async_trait::async_trait;
    use chrono::{TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::{
        bars::{
            BarsUseCases, SharedBarsRepository, SharedUsStockBarsWriteLock, UsStockBarPage,
            UsStockBarQuery, UsStockBarSource, UsStockBarSourceError, UsStockBarsIngestStats,
            UsStockBarsIngestUseCases, UsStockBarsWriteLock, UsStockBarsWriteLockGuard,
            UsStockSipBarRefetchConfig, UsStockSplitBatch, UsStockSplitQuery,
            us_stock_sip_bars_history_cutoff,
        },
        persistence::PersistenceError,
        unit_of_work::{FakeUnitOfWork, SharedUnitOfWork},
    };

    #[derive(Default)]
    struct FakeSource {
        calls: Mutex<Vec<UsStockBarQuery>>,
        bars: Mutex<Vec<Bar>>,
    }

    #[async_trait]
    impl UsStockBarSource for FakeSource {
        async fn fetch_page(
            &self,
            query: &UsStockBarQuery,
            _deadline: Instant,
        ) -> Result<UsStockBarPage, UsStockBarSourceError> {
            self.calls.lock().await.push(query.clone());
            let configured_bars = self.bars.lock().await.clone();
            let bars = if configured_bars.is_empty() {
                query
                    .instrument_ids
                    .first()
                    .map(|instrument_id| vec![bar(instrument_id, query.timeframe, query.to)])
                    .unwrap_or_default()
            } else {
                configured_bars
                    .into_iter()
                    .filter(|bar| {
                        query.instrument_ids.contains(&bar.instrument_id)
                            && bar.timeframe == query.timeframe
                            && bar.timestamp >= query.from
                            && bar.timestamp <= query.to
                    })
                    .collect()
            };
            Ok(UsStockBarPage {
                bars,
                next_page_token: None,
                skipped_instruments: Vec::new(),
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

    impl UsStockBarsWriteLockGuard for ImmediateWriteLockGuard {}

    #[async_trait]
    impl UsStockBarsWriteLock for ImmediateWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: Duration,
        ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, PersistenceError> {
            Ok(Some(Box::new(ImmediateWriteLockGuard)))
        }
    }

    struct Harness {
        use_cases: UsStockBarsIngestUseCases,
        repository: Arc<crate::bars::FakeBarsRepository>,
        source: FakeSource,
    }

    fn harness() -> Harness {
        let repository = Arc::new(crate::bars::FakeBarsRepository::new());
        let repository_trait: SharedBarsRepository = repository.clone();
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let unit_of_work_trait: SharedUnitOfWork = unit_of_work;
        let write_lock: SharedUsStockBarsWriteLock = Arc::new(ImmediateWriteLock);
        Harness {
            use_cases: UsStockBarsIngestUseCases::new(
                BarsUseCases::new(unit_of_work_trait, repository_trait),
                write_lock,
            ),
            repository,
            source: FakeSource::default(),
        }
    }

    fn without_runtime_throttle_time(mut stats: UsStockBarsIngestStats) -> UsStockBarsIngestStats {
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
                without_runtime_throttle_time(result.stats),
                queries,
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
                        write_throttle_wait_ms: 0,
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
}
