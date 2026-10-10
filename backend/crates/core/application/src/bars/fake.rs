use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::bar::{Bar, Timeframe};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{BarsRepository, BarsRepositoryError};
use super::types::{BarsByInstrumentsQuery, BarsQuery, UsStockBarTarget};

#[derive(Default)]
pub struct FakeBarsRepository {
    pub bars: Mutex<Vec<Bar>>,
    pub minute_bars: Mutex<Vec<Bar>>,
    pub ingested_dates: Mutex<HashSet<NaiveDate>>,
    pub instruments: Mutex<HashSet<String>>,
    pub us_stock_targets: Mutex<HashSet<String>>,
    pub write_transaction_ids: Mutex<Vec<Uuid>>,
}

impl FakeBarsRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn seed_bars(&self, bars: Vec<Bar>) {
        let (minute_bars, bars): (Vec<_>, Vec<_>) = bars
            .into_iter()
            .partition(|bar| bar.timeframe == Timeframe::Minute);
        self.bars.lock().await.extend(bars);
        self.minute_bars.lock().await.extend(minute_bars);
    }

    pub async fn seed_ingested_dates(&self, dates: HashSet<NaiveDate>) {
        self.ingested_dates.lock().await.extend(dates);
    }

    pub async fn seed_us_stock_targets(&self, instrument_ids: Vec<String>) {
        self.us_stock_targets.lock().await.extend(instrument_ids);
    }

    async fn all_bars(&self) -> Vec<Bar> {
        let mut bars = self.bars.lock().await.clone();
        bars.extend(self.minute_bars.lock().await.iter().cloned());
        bars
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

    async fn find_us_stock_bar_targets(
        &self,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
        let target_ids = self.us_stock_targets.lock().await.clone();
        let daily_bars = self.bars.lock().await;
        let minute_bars = self.minute_bars.lock().await;
        let mut targets = target_ids
            .into_iter()
            .map(|instrument_id| UsStockBarTarget {
                latest_daily_bar: daily_bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Daily
                    })
                    .max_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                latest_minute_bar: minute_bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Minute
                    })
                    .max_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                earliest_daily_bar: daily_bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Daily
                    })
                    .min_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                earliest_minute_bar: minute_bars
                    .iter()
                    .filter(|bar| {
                        bar.instrument_id == instrument_id && bar.timeframe == Timeframe::Minute
                    })
                    .min_by_key(|bar| bar.timestamp)
                    .map(|bar| bar.timestamp),
                instrument_id,
            })
            .collect::<Vec<_>>();
        targets.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
        Ok(targets)
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
        let mut bars = self.bars.lock().await;
        upsert_bar_batch(&mut bars, new_bars);
        Ok(())
    }

    async fn upsert_minute_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        new_bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError> {
        self.record_transaction(transaction).await?;
        let mut minute_bars = self.minute_bars.lock().await;
        upsert_bar_batch(&mut minute_bars, new_bars);
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
