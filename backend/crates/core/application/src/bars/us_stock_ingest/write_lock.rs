use chrono::{DateTime, Utc};

use super::minute_ingest_window::{
    all_us_stock_minute_targets_complete_for_closed_window, is_us_stock_minute_ingest_window,
};
use super::request_budget::RequestBudget;
use super::{MAX_REQUESTS_PER_RUN, UsStockBarTimeframe};
use crate::bars::error::{BarsUseCaseError, UsStockBarsIngestError};
use crate::bars::types::{UsStockBarsIngestResult, UsStockBarsIngestStats};
use crate::bars::us_stock_source::UsStockBarSource;
use crate::bars::use_cases::UsStockBarsIngestUseCases;
use crate::bars::write_lock::UsStockBarsWriteLockGuard;
use crate::bars::{US_STOCK_DAILY_BARS_MAX_RUN_DURATION, US_STOCK_INTRADAY_BARS_MAX_RUN_DURATION};

impl UsStockBarsIngestUseCases {
    pub async fn ingest_us_stock_daily_bars(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        self.ingest_us_stock_bars_with_write_lock(
            source,
            now,
            UsStockBarTimeframe::Daily,
            RequestBudget::new(
                MAX_REQUESTS_PER_RUN,
                US_STOCK_DAILY_BARS_MAX_RUN_DURATION,
                self.clock.instant(),
            ),
        )
        .await
    }

    pub async fn ingest_us_stock_minute_bars(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        self.ingest_us_stock_minute_bars_with_budget(
            source,
            now,
            RequestBudget::new(
                MAX_REQUESTS_PER_RUN,
                US_STOCK_INTRADAY_BARS_MAX_RUN_DURATION,
                self.clock.instant(),
            ),
        )
        .await
    }

    pub async fn ingest_us_stock_hourly_bars(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        self.ingest_us_stock_bars_with_write_lock(
            source,
            now,
            UsStockBarTimeframe::Hourly,
            RequestBudget::new(
                MAX_REQUESTS_PER_RUN,
                US_STOCK_INTRADAY_BARS_MAX_RUN_DURATION,
                self.clock.instant(),
            ),
        )
        .await
    }

    pub(super) async fn ingest_us_stock_minute_bars_with_budget(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
        budget: RequestBudget,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        self.ingest_us_stock_bars_with_write_lock(source, now, UsStockBarTimeframe::Minute, budget)
            .await
    }

    async fn ingest_us_stock_bars_with_write_lock(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
        timeframe: UsStockBarTimeframe,
        mut budget: RequestBudget,
    ) -> Result<UsStockBarsIngestResult, UsStockBarsIngestError> {
        if timeframe == UsStockBarTimeframe::Minute {
            let skip_closed_range = self.should_skip_closed_minute_range_before_lock(now).await;
            match skip_closed_range {
                Ok(true) => {
                    return Ok(UsStockBarsIngestResult::new(
                        UsStockBarsIngestStats::default(),
                        None,
                    ));
                }
                Ok(false) => {}
                Err(error) => {
                    return Err(UsStockBarsIngestError::new(
                        UsStockBarsIngestResult::new(UsStockBarsIngestStats::default(), None),
                        error,
                    ));
                }
            }
        }

        let mut lock_wait_stats = UsStockBarsIngestStats::default();
        let write_lock_result = self
            .acquire_us_stock_bars_write_lock(&mut budget, &mut lock_wait_stats)
            .await;
        let write_lock_guard = match write_lock_result {
            Ok(Some(guard)) => guard,
            Ok(None) => return Ok(UsStockBarsIngestResult::new(lock_wait_stats, None)),
            Err(error) => {
                return Err(UsStockBarsIngestError::new(
                    UsStockBarsIngestResult::new(lock_wait_stats, None),
                    error,
                ));
            }
        };

        let (mut stats, ingest_error) = match self
            .ingest_us_stock_bars_for_timeframe(source, now, timeframe, budget)
            .await
        {
            Ok(stats) => (stats, None),
            Err(failure) => {
                let (stats, error) = *failure;
                (stats, Some(error))
            }
        };
        stats.write_lock_wait_ms = lock_wait_stats.write_lock_wait_ms;

        if timeframe == UsStockBarTimeframe::Minute
            && stats.symbols_attempted > 0
            && let Err(error) = self
                .bars
                .repository
                .compress_bars_older_than_policy(now)
                .await
        {
            let compression_error = error.to_string();
            stats.compression_error = Some(compression_error.clone());
            if ingest_error.is_some() {
                tracing::error!(error = %compression_error, "米国株の分足取り込み後に古い bars chunk を圧縮できませんでした");
            }
        }

        let result = UsStockBarsIngestResult::new(stats, Some(write_lock_guard));
        match ingest_error {
            Some(error) => Err(UsStockBarsIngestError::new(result, error)),
            None => Ok(result),
        }
    }

    async fn should_skip_closed_minute_range_before_lock(
        &self,
        now: DateTime<Utc>,
    ) -> Result<bool, BarsUseCaseError> {
        if is_us_stock_minute_ingest_window(now) {
            // 場中は分足取得の対象になるため、事前照会を省く。
            return Ok(false);
        }

        let targets = self
            .bars
            .repository
            .find_us_stock_minute_bar_targets()
            .await?;

        Ok(all_us_stock_minute_targets_complete_for_closed_window(
            now,
            targets
                .iter()
                .map(|target| target.minute_history_completed_through),
        ))
    }

    pub(super) async fn acquire_us_stock_bars_write_lock(
        &self,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<Option<Box<dyn UsStockBarsWriteLockGuard>>, BarsUseCaseError> {
        let acquire_started_at = self.clock.instant();
        if !budget.can_start_request(acquire_started_at) {
            stats.interrupted_reason = budget.interrupted_reason.map(str::to_owned);
            return Ok(None);
        }

        let wait_timeout = budget
            .deadline()
            .saturating_duration_since(acquire_started_at);
        let lock = tokio::select! {
            biased;
            result = self.us_stock_bars_write_lock.acquire(wait_timeout) => result,
            _ = self.clock.sleep(wait_timeout) => Ok(None),
        };
        let completed_at = self.clock.instant();
        let waited = completed_at.saturating_duration_since(acquire_started_at);
        stats.write_lock_wait_ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX);
        let lock = lock?;

        match lock {
            Some(guard) if budget.can_start_request(completed_at) => Ok(Some(guard)),
            Some(_) => {
                stats.interrupted_reason = budget.interrupted_reason.map(str::to_owned);
                Ok(None)
            }
            None => {
                budget.mark_time_budget_exhausted();
                stats.interrupted_reason = budget.interrupted_reason.map(str::to_owned);
                Ok(None)
            }
        }
    }
}
