use std::collections::HashSet;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use core_domain::bar::Bar;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

use super::types::{
    BarsByInstrumentsQuery, BarsQuery, DailyBarAdjustmentFactor, UsStockBarTarget,
    UsStockSipBarRefetchCursor, UsStockSipBarRefetchTarget,
};

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

    async fn find_earliest_us_stock_minute_bar_before(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<Option<DateTime<Utc>>, BarsRepositoryError>;

    async fn find_us_stock_instrument_ids(&self) -> Result<Vec<String>, BarsRepositoryError>;

    async fn downsample_us_stock_minute_bar_range(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_id: &str,
        from: DateTime<Utc>,
        until: DateTime<Utc>,
        cutoff: DateTime<Utc>,
    ) -> Result<super::types::UsStockMinuteBarsDownsampleStats, BarsRepositoryError>;

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

    async fn find_us_stock_minute_bar_targets(
        &self,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError>;

    async fn find_us_stock_daily_bar_targets(
        &self,
    ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError>;

    async fn find_pending_us_stock_sip_bar_refetch_targets(
        &self,
        max_symbols: Option<usize>,
    ) -> Result<Vec<UsStockSipBarRefetchTarget>, BarsRepositoryError>;

    async fn save_us_stock_sip_bar_refetch_cursor(
        &self,
        transaction: &UnitOfWorkTransaction,
        instrument_id: &str,
        cursor: Option<UsStockSipBarRefetchCursor>,
    ) -> Result<(), BarsRepositoryError>;

    async fn mark_us_stock_sip_bar_refetch_completed(
        &self,
        instrument_id: &str,
    ) -> Result<(), BarsRepositoryError>;

    async fn mark_us_stock_sip_bar_refetch_skipped(
        &self,
        instrument_id: &str,
        reason: &str,
    ) -> Result<(), BarsRepositoryError>;

    async fn reserve_us_stock_sip_refetch_rows(
        &self,
        transaction: &UnitOfWorkTransaction,
        rows: u64,
        max_rows_per_hour: u64,
    ) -> Result<bool, BarsRepositoryError>;

    async fn mark_us_stock_hourly_bar_history_completed(
        &self,
        instrument_ids: &[String],
        completed_through: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError>;

    async fn mark_us_stock_minute_bar_history_completed(
        &self,
        instrument_ids: &[String],
        completed_through: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError>;

    async fn compress_bars_older_than_policy(
        &self,
        now: DateTime<Utc>,
    ) -> Result<(), BarsRepositoryError>;

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

    async fn mark_ingested(
        &self,
        transaction: &UnitOfWorkTransaction,
        date: NaiveDate,
    ) -> Result<(), BarsRepositoryError>;
}

pub type SharedBarsRepository = std::sync::Arc<dyn BarsRepository + Send + Sync>;
