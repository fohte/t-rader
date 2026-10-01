use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::bar::Bar;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::unit_of_work::{FakeTransaction, UnitOfWorkTransaction};

use super::repository::{BarsRepository, BarsRepositoryError};
use super::types::{BarsByInstrumentsQuery, BarsQuery};

#[derive(Default)]
pub struct FakeBarsRepository {
    pub bars: Mutex<Vec<Bar>>,
    pub ingested_dates: Mutex<HashSet<NaiveDate>>,
    pub instruments: Mutex<HashSet<String>>,
    pub write_transaction_ids: Mutex<Vec<Uuid>>,
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
}

#[async_trait]
impl BarsRepository for FakeBarsRepository {
    async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError> {
        let mut bars: Vec<Bar> = self
            .bars
            .lock()
            .await
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
        let mut bars: Vec<Bar> = self
            .bars
            .lock()
            .await
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
        Ok(self
            .bars
            .lock()
            .await
            .iter()
            .filter(|bar| {
                bar.instrument_id == instrument_id && bar.timeframe.to_string() == timeframe
            })
            .max_by(|left, right| left.timestamp.cmp(&right.timestamp))
            .cloned())
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
