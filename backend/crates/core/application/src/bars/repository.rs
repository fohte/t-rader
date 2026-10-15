use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::bar::Bar;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{BarsByInstrumentsQuery, BarsQuery, DailyBarAdjustmentFactor, UsStockBarTarget};

#[derive(Debug, Error)]
pub enum BarsRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait BarsRepository: Send + Sync {
    async fn find_bars(&self, query: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError>;

    async fn find_bars_by_instruments(
        &self,
        query: BarsByInstrumentsQuery,
    ) -> Result<Vec<Bar>, BarsRepositoryError>;

    async fn find_latest_bar(
        &self,
        instrument_id: &str,
        timeframe: &str,
    ) -> Result<Option<Bar>, BarsRepositoryError>;

    async fn find_daily_adjustment_factors_from(
        &self,
        instrument_id: &str,
        from: NaiveDate,
    ) -> Result<Vec<DailyBarAdjustmentFactor>, BarsRepositoryError>;

    async fn find_us_stock_bar_targets(&self)
    -> Result<Vec<UsStockBarTarget>, BarsRepositoryError>;

    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<HashSet<NaiveDate>, BarsRepositoryError>;

    async fn ensure_instruments_exist(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_ids: &HashSet<String>,
    ) -> Result<(), BarsRepositoryError>;

    async fn upsert_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError>;

    async fn upsert_minute_bars(
        &self,
        transaction: &UnitOfWorkTransaction,
        bars: Vec<Bar>,
    ) -> Result<(), BarsRepositoryError>;

    async fn mark_ingested(
        &self,
        transaction: &UnitOfWorkTransaction,
        date: NaiveDate,
    ) -> Result<(), BarsRepositoryError>;
}

pub type SharedBarsRepository = std::sync::Arc<dyn BarsRepository + Send + Sync>;
