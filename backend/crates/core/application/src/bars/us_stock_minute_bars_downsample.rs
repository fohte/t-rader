use chrono::{DateTime, Duration, Utc};

use super::error::BarsUseCaseError;
use super::hourly_bar_bucket_start;
use super::types::UsStockMinuteBarsDownsampleStats;
use super::us_stock_hourly_history_cutoff;
use super::use_cases::BarsUseCases;

const DOWNSAMPLE_BATCH_DAYS: i64 = 7;

impl BarsUseCases {
    pub async fn downsample_old_us_stock_minute_bars(
        &self,
        now: DateTime<Utc>,
    ) -> Result<UsStockMinuteBarsDownsampleStats, BarsUseCaseError> {
        let cutoff = us_stock_hourly_history_cutoff(now);
        let Some(oldest_bar) = self
            .repository
            .find_earliest_us_stock_minute_bar_before(cutoff)
            .await?
        else {
            return Ok(UsStockMinuteBarsDownsampleStats::default());
        };

        let mut batch_from = hourly_bar_bucket_start(oldest_bar);
        let mut stats = UsStockMinuteBarsDownsampleStats::default();
        while batch_from < cutoff {
            let batch_until = batch_from
                .checked_add_signed(Duration::days(DOWNSAMPLE_BATCH_DAYS))
                .unwrap_or(cutoff);
            let transaction = self.unit_of_work.begin().await?;
            let batch_stats = self
                .repository
                .downsample_us_stock_minute_bar_range(&transaction, batch_from, batch_until, cutoff)
                .await?;
            self.unit_of_work.commit(transaction).await?;

            stats.hourly_bars_inserted += batch_stats.hourly_bars_inserted;
            stats.minute_bars_deleted += batch_stats.minute_bars_deleted;
            batch_from = batch_until;
        }

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use std::sync::Arc;

    use chrono::{DateTime, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rust_decimal::Decimal;

    use crate::persistence::PersistenceError;
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::super::{BarsUseCases, FakeBarsRepository};

    struct TestBarsWriteLockGuard;

    impl crate::bars::UsStockBarsWriteLockGuard for TestBarsWriteLockGuard {}

    struct TestBarsWriteLock;

    #[async_trait]
    impl crate::bars::UsStockBarsWriteLock for TestBarsWriteLock {
        async fn acquire(
            &self,
            _wait_timeout: std::time::Duration,
        ) -> Result<Option<Box<dyn crate::bars::UsStockBarsWriteLockGuard>>, PersistenceError>
        {
            Ok(Some(Box::new(TestBarsWriteLockGuard)))
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

    #[tokio::test]
    async fn downsample_processes_aged_minutes_in_hour_aligned_weekly_transactions() {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeBarsRepository::new());
        repository
            .seed_bars(vec![
                minute_bar("US:DEMO-ALPHA", date("2038-12-20T03:05:00Z")),
                minute_bar("US:DEMO-ALPHA", date("2038-12-29T10:05:00Z")),
                minute_bar("US:DEMO-ALPHA", date("2039-01-01T12:05:00Z")),
                minute_bar("JP-DEMO", date("2038-12-20T03:05:00Z")),
            ])
            .await;
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let use_cases = BarsUseCases::new(
            shared_unit_of_work,
            repository.clone(),
            Arc::new(TestBarsWriteLock),
        );

        let result = use_cases
            .downsample_old_us_stock_minute_bars(date("2040-01-01T12:00:00Z"))
            .await;

        assert_eq!(
            (
                result.map_err(|_| ()),
                repository.minute_bar_downsample_ranges.lock().await.clone(),
                repository
                    .minute_bar_downsample_cutoffs
                    .lock()
                    .await
                    .clone(),
                unit_of_work.committed.lock().await.len(),
            ),
            (
                Ok(Default::default()),
                vec![
                    (date("2038-12-20T03:00:00Z"), date("2038-12-27T03:00:00Z"),),
                    (date("2038-12-27T03:00:00Z"), date("2039-01-03T03:00:00Z"),),
                ],
                vec![date("2039-01-01T12:00:00Z"), date("2039-01-01T12:00:00Z"),],
                2,
            ),
        );
    }
}
