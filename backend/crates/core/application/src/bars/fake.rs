use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use core_domain::bar::{Bar, Timeframe};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{BarsRepository, BarsRepositoryError};
use super::types::{
    BarsByInstrumentsQuery, BarsQuery, DailyBarAdjustmentFactor, UsStockBarTarget,
    UsStockMinuteBarsDownsampleStats,
};

#[derive(Default)]
pub struct FakeBarsRepository {
    pub bars: Mutex<Vec<Bar>>,
    pub ingested_dates: Mutex<HashSet<NaiveDate>>,
    pub instruments: Mutex<HashSet<String>>,
    pub us_stock_instrument_ids: Mutex<HashSet<String>>,
    pub us_stock_targets: Mutex<HashSet<String>>,
    pub us_stock_daily_targets: Mutex<HashSet<String>>,
    pub us_stock_minute_targets: Mutex<HashSet<String>>,
    pub us_stock_sip_refetch_completed: Mutex<HashSet<String>>,
    pub us_stock_sip_refetch_skipped: Mutex<HashMap<String, String>>,
    pub us_stock_sip_refetch_rows_reserved: Mutex<u64>,
    pub us_stock_hourly_bar_history_completed_through: Mutex<HashMap<String, DateTime<Utc>>>,
    pub bar_compression_times: Mutex<Vec<DateTime<Utc>>>,
    pub write_transaction_ids: Mutex<Vec<Uuid>>,
    pub daily_adjustment_factors: Mutex<HashMap<String, Vec<DailyBarAdjustmentFactor>>>,
    pub daily_adjustment_factor_queries: Mutex<Vec<(String, NaiveDate)>>,
    pub minute_bar_downsample_instruments: Mutex<Vec<String>>,
    pub minute_bar_downsample_ranges: Mutex<Vec<(DateTime<Utc>, DateTime<Utc>)>>,
    pub minute_bar_downsample_cutoffs: Mutex<Vec<DateTime<Utc>>>,
    minute_bar_downsample_stats: Mutex<UsStockMinuteBarsDownsampleStats>,
    minute_bar_downsample_failure: Mutex<Option<(String, String)>>,
    hourly_upsert_error: Mutex<Option<String>>,
    bar_compression_error: Mutex<Option<String>>,
}

impl FakeBarsRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn seed_bars(&self, bars: Vec<Bar>) {
        self.bars.lock().await.extend(bars);
    }

    pub async fn seed_ingested_dates(&self, dates: HashSet<NaiveDate>) {
        self.ingested_dates.lock().await.extend(dates);
    }

    pub async fn seed_us_stock_targets(&self, instrument_ids: Vec<String>) {
        self.us_stock_targets.lock().await.extend(instrument_ids);
    }

    pub async fn seed_us_stock_instrument_ids(&self, instrument_ids: Vec<String>) {
        self.us_stock_instrument_ids
            .lock()
            .await
            .extend(instrument_ids);
    }

    pub async fn seed_us_stock_daily_targets(&self, instrument_ids: Vec<String>) {
        self.us_stock_daily_targets
            .lock()
            .await
            .extend(instrument_ids);
    }

    pub async fn seed_us_stock_minute_targets(&self, instrument_ids: Vec<String>) {
        self.us_stock_minute_targets
            .lock()
            .await
            .extend(instrument_ids);
    }

    pub async fn seed_daily_adjustment_factors(
        &self,
        instrument_id: impl Into<String>,
        factors: Vec<DailyBarAdjustmentFactor>,
    ) {
        self.daily_adjustment_factors
            .lock()
            .await
            .entry(instrument_id.into())
            .or_default()
            .extend(factors);
    }

    pub async fn fail_hourly_upserts_with(&self, message: impl Into<String>) {
        *self.hourly_upsert_error.lock().await = Some(message.into());
    }

    pub async fn fail_minute_bar_downsampling_for(
        &self,
        instrument_id: impl Into<String>,
        message: impl Into<String>,
    ) {
        *self.minute_bar_downsample_failure.lock().await =
            Some((instrument_id.into(), message.into()));
    }

    pub async fn set_minute_bar_downsample_stats(&self, stats: UsStockMinuteBarsDownsampleStats) {
        *self.minute_bar_downsample_stats.lock().await = stats;
    }

    pub async fn fail_bar_compression_with(&self, message: impl Into<String>) {
        *self.bar_compression_error.lock().await = Some(message.into());
    }

    async fn all_bars(&self) -> Vec<Bar> {
        self.bars.lock().await.clone()
    }

    async fn find_targets(
        &self,
        target_ids: HashSet<String>,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
        let bars = self.bars.lock().await;
        let hourly_progress = self
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await;
        let mut targets = target_ids
            .into_iter()
            .map(|instrument_id| UsStockBarTarget {
                latest_daily_bar: bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Daily
                    })
                    .max_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                latest_minute_bar: bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Minute
                    })
                    .max_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                latest_hourly_bar: bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Hourly
                    })
                    .max_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                hourly_history_completed_through: hourly_progress.get(&instrument_id).copied(),
                earliest_daily_bar: bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Daily
                    })
                    .min_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                earliest_minute_bar: bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Minute
                    })
                    .min_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                earliest_hourly_bar: bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Hourly
                    })
                    .min_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                instrument_id,
            })
            .collect::<Vec<_>>();
        targets.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
        Ok(targets)
    }
}

#[async_trait]
impl BarsRepository for FakeBarsRepository {
    async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError> {
        let all_bars = self.all_bars().await;
        let mut bars: Vec<Bar> = all_bars
            .iter()
            .filter(|bar| {
                let timestamp = bar.timestamp.fixed_offset();
                bar.instrument_id == query.instrument_id
                    && bar.timeframe.to_string() == query.timeframe
                    && query.from.as_ref().is_none_or(|from| timestamp >= *from)
                    && query.to.as_ref().is_none_or(|to| timestamp <= *to)
            })
            .cloned()
            .collect();
        bars.sort_by_key(|bar| bar.timestamp);
        Ok(bars)
    }

    async fn find_bars_by_instruments(
        &self,
        query: BarsByInstrumentsQuery,
    ) -> Result<Vec<Bar>, BarsRepositoryError> {
        let all_bars = self.all_bars().await;
        let mut bars: Vec<Bar> = all_bars
            .iter()
            .filter(|bar| {
                let timestamp = bar.timestamp.fixed_offset();
                query.instrument_ids.contains(&bar.instrument_id)
                    && bar.timeframe.to_string() == query.timeframe
                    && query.from.as_ref().is_none_or(|from| timestamp >= *from)
                    && query.to.as_ref().is_none_or(|to| timestamp <= *to)
            })
            .cloned()
            .collect();
        bars.sort_by_key(|bar| (bar.instrument_id.clone(), bar.timestamp));
        Ok(bars)
    }

    async fn find_earliest_us_stock_minute_bar_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, BarsRepositoryError> {
        Ok(self
            .bars
            .lock()
            .await
            .iter()
            .filter(|bar| {
                bar.instrument_id.starts_with("US:")
                    && bar.timeframe == Timeframe::Minute
                    && bar.timestamp < cutoff
            })
            .map(|bar| bar.timestamp)
            .min())
    }

    async fn find_us_stock_instrument_ids(&self) -> Result<Vec<String>, BarsRepositoryError> {
        let mut instrument_ids = self
            .us_stock_instrument_ids
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        instrument_ids.sort();
        Ok(instrument_ids)
    }

    async fn downsample_us_stock_minute_bar_range(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_id: &str,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
        cutoff: DateTime<Utc>,
    ) -> Result<UsStockMinuteBarsDownsampleStats, BarsRepositoryError> {
        self.record_transaction(transaction).await?;
        self.minute_bar_downsample_instruments
            .lock()
            .await
            .push(instrument_id.to_owned());
        self.minute_bar_downsample_ranges
            .lock()
            .await
            .push((from, until));
        self.minute_bar_downsample_cutoffs.lock().await.push(cutoff);
        let failure_message = self
            .minute_bar_downsample_failure
            .lock()
            .await
            .as_ref()
            .filter(|(failed_instrument_id, _)| failed_instrument_id == instrument_id)
            .map(|(_, message)| message.clone());
        if let Some(message) = failure_message {
            return Err(BarsRepositoryError::Database(PersistenceError::Database(
                message,
            )));
        }
        Ok(*self.minute_bar_downsample_stats.lock().await)
    }

    async fn find_latest_bar(
        &self,
        instrument_id: &str,
        timeframe: &str,
    ) -> Result<Option<Bar>, BarsRepositoryError> {
        let all_bars = self.all_bars().await;
        Ok(all_bars
            .iter()
            .filter(|bar| {
                bar.instrument_id == instrument_id && bar.timeframe.to_string() == timeframe
            })
            .max_by(|left, right| left.timestamp.cmp(&right.timestamp))
            .cloned())
    }

    async fn find_daily_adjustment_factors_from(
        &self,
        instrument_id: &str,
        from: NaiveDate,
    ) -> Result<Vec<DailyBarAdjustmentFactor>, BarsRepositoryError> {
        self.daily_adjustment_factor_queries
            .lock()
            .await
            .push((instrument_id.to_owned(), from));
        Ok(self
            .daily_adjustment_factors
            .lock()
            .await
            .get(instrument_id)
            .into_iter()
            .flatten()
            .filter(|factor| factor.date >= from)
            .copied()
            .collect())
    }

    async fn find_us_stock_minute_bar_targets(
        &self,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
        let mut target_ids = self.us_stock_targets.lock().await.clone();
        target_ids.extend(self.us_stock_minute_targets.lock().await.iter().cloned());
        self.find_targets(target_ids).await
    }

    async fn find_us_stock_daily_bar_targets(
        &self,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
        let mut target_ids = self.us_stock_targets.lock().await.clone();
        target_ids.extend(self.us_stock_daily_targets.lock().await.iter().cloned());
        self.find_targets(target_ids).await
    }

    async fn find_pending_us_stock_sip_bar_refetch_targets(
        &self,
        max_symbols: Option<usize>,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
        let mut target_ids = self.us_stock_targets.lock().await.clone();
        target_ids.extend(self.us_stock_daily_targets.lock().await.iter().cloned());
        target_ids.extend(self.us_stock_minute_targets.lock().await.iter().cloned());
        let completed = self.us_stock_sip_refetch_completed.lock().await;
        let skipped = self.us_stock_sip_refetch_skipped.lock().await;
        target_ids.retain(|instrument_id| {
            !completed.contains(instrument_id) && !skipped.contains_key(instrument_id)
        });
        drop(completed);
        drop(skipped);
        let mut targets = self.find_targets(target_ids).await?;
        if let Some(max_symbols) = max_symbols {
            targets.truncate(max_symbols);
        }
        Ok(targets)
    }

    async fn mark_us_stock_sip_bar_refetch_completed(
        &self,
        instrument_id: &str,
    ) -> Result<(), BarsRepositoryError> {
        self.us_stock_sip_refetch_completed
            .lock()
            .await
            .insert(instrument_id.to_owned());
        self.us_stock_sip_refetch_skipped
            .lock()
            .await
            .remove(instrument_id);
        Ok(())
    }

    async fn mark_us_stock_sip_bar_refetch_skipped(
        &self,
        instrument_id: &str,
        reason: &str,
    ) -> Result<(), BarsRepositoryError> {
        self.us_stock_sip_refetch_skipped
            .lock()
            .await
            .insert(instrument_id.to_owned(), reason.to_owned());
        Ok(())
    }

    async fn reserve_us_stock_sip_refetch_rows(
        &self,
        transaction: &UnitOfWorkTransaction,
        rows: u64,
        max_rows_per_hour: u64,
    ) -> Result<bool, BarsRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut reserved = self.us_stock_sip_refetch_rows_reserved.lock().await;
        if reserved.saturating_add(rows) > max_rows_per_hour {
            return Ok(false);
        }
        *reserved += rows;
        Ok(true)
    }

    async fn mark_us_stock_hourly_bar_history_completed(
        &self,
        instrument_ids: &[String],
        completed_through: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError> {
        let mut progress = self
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await;
        for instrument_id in instrument_ids {
            progress
                .entry(instrument_id.clone())
                .and_modify(|saved| *saved = (*saved).max(completed_through))
                .or_insert(completed_through);
        }
        Ok(())
    }

    async fn compress_bars_older_than_policy(
        &self,
        now: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError> {
        self.bar_compression_times.lock().await.push(now);
        if let Some(message) = self.bar_compression_error.lock().await.take() {
            return Err(BarsRepositoryError::Database(PersistenceError::Database(
                message,
            )));
        }
        Ok(())
    }

    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<HashSet<NaiveDate>, BarsRepositoryError> {
        Ok(self
            .ingested_dates
            .lock()
            .await
            .iter()
            .filter(|date| **date >= from)
            .copied()
            .collect())
    }

    async fn ensure_instruments_exist(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_ids: &HashSet<String>,
    ) -> Result<(), BarsRepositoryError> {
        self.record_transaction(transaction).await?;
        self.instruments.lock().await.extend(instrument_ids.clone());
        Ok(())
    }

    async fn upsert_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        new_bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError> {
        self.record_transaction(transaction).await?;
        if new_bars
            .iter()
            .any(|bar| bar.timeframe == Timeframe::Hourly)
            && let Some(message) = self.hourly_upsert_error.lock().await.take()
        {
            return Err(BarsRepositoryError::Database(PersistenceError::Database(
                message,
            )));
        }
        let mut bars = self.bars.lock().await;
        upsert_bar_batch(&mut bars, new_bars);
        Ok(())
    }

    async fn mark_ingested(
        &self,
        transaction: &UnitOfWorkTransaction,
        date: NaiveDate,
    ) -> Result<(), BarsRepositoryError> {
        self.record_transaction(transaction).await?;
        self.ingested_dates.lock().await.insert(date);
        Ok(())
    }
}

fn upsert_bar_batch(bars: &mut Vec<Bar>, new_bars: Vec<Bar>) {
    for new_bar in new_bars {
        if let Some(existing) = bars.iter_mut().find(|bar| {
            bar.instrument_id == new_bar.instrument_id
                && bar.timeframe == new_bar.timeframe
                && bar.timestamp == new_bar.timestamp
        }) {
            *existing = new_bar;
        } else {
            bars.push(new_bar);
        }
    }
}

impl FakeBarsRepository {
    async fn record_transaction(
        &self,
        transaction: &UnitOfWorkTransaction,
    ) -> Result<(), BarsRepositoryError> {
        let id = transaction
            .downcast_ref::<FakeTransaction>()
            .map(|transaction| transaction.id)
            .ok_or(BarsRepositoryError::InvalidTransaction)?;
        self.write_transaction_ids.lock().await.push(id);
        Ok(())
    }
}
