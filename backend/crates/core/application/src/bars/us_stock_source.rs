use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockBarQuery {
    pub instrument_ids: Vec<String>,
    pub timeframe: Timeframe,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UsStockBarPage {
    pub bars: Vec<Bar>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockSplitQuery {
    pub instrument_ids: Vec<String>,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsStockSplit {
    pub instrument_id: String,
    pub ex_date: NaiveDate,
    pub old_rate: Decimal,
    pub new_rate: Decimal,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum UsStockBarSourceError {
    #[error("US stock bar source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait UsStockBarSource: Send + Sync {
    async fn fetch_page(
        &self,
        query: &UsStockBarQuery,
    ) -> Result<UsStockBarPage, UsStockBarSourceError>;

    async fn fetch_splits(
        &self,
        query: &UsStockSplitQuery,
    ) -> Result<Vec<UsStockSplit>, UsStockBarSourceError>;
}

pub type SharedUsStockBarSource = Arc<dyn UsStockBarSource>;
