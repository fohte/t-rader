use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::persistence::PersistenceError;

pub const SCREENER_PAGE_SIZE: u32 = 100;

/// 保存条件を JSON で保ち、新しい条件を追加しても既存の設定を読み込めるようにする。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenerFilters {
    pub markets: Vec<String>,
    pub sectors: Vec<String>,
    pub per: Option<ScreenerRange>,
    pub pbr: Option<ScreenerRange>,
    /// パーセント単位。
    pub roe: Option<ScreenerRange>,
    /// パーセント単位。
    pub dividend_yield: Option<ScreenerRange>,
    /// 億円単位。
    pub market_cap: Option<ScreenerRange>,
}

impl ScreenerFilters {
    pub fn validate(&self) -> Result<(), ScreenerFilterValidationError> {
        for (field, range) in [
            ("per", self.per.as_ref()),
            ("pbr", self.pbr.as_ref()),
            ("roe", self.roe.as_ref()),
            ("dividend_yield", self.dividend_yield.as_ref()),
            ("market_cap", self.market_cap.as_ref()),
        ] {
            if let Some(range) = range
                && matches!((range.min, range.max), (Some(min), Some(max)) if min > max)
            {
                return Err(ScreenerFilterValidationError::ReversedRange { field });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenerRange {
    pub min: Option<Decimal>,
    pub max: Option<Decimal>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScreenerFilterValidationError {
    #[error("{field} minimum must be less than or equal to maximum")]
    ReversedRange { field: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScreenerSortField {
    Id,
    Name,
    Market,
    Sector,
    Close,
    Per,
    Pbr,
    Roe,
    DividendYield,
    #[default]
    MarketCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScreenerSortDirection {
    Asc,
    #[default]
    Desc,
}

/// 保存条件で選んだ並び替えを復元できるようにする。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScreenerSort {
    #[serde(default)]
    pub field: ScreenerSortField,
    #[serde(default)]
    pub direction: ScreenerSortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenerQuery {
    pub filters: ScreenerFilters,
    pub sort: ScreenerSort,
    pub page: u32,
}

impl Default for ScreenerQuery {
    fn default() -> Self {
        Self {
            filters: ScreenerFilters::default(),
            sort: ScreenerSort::default(),
            page: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenerStock {
    pub id: String,
    pub name: String,
    pub market: Option<String>,
    pub sector: Option<String>,
    pub close: Option<Decimal>,
    pub per: Option<Decimal>,
    pub pbr: Option<Decimal>,
    pub roe: Option<Decimal>,
    pub dividend_yield: Option<Decimal>,
    pub market_cap: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenerSector {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScreenerPage {
    pub items: Vec<ScreenerStock>,
    pub total_count: i64,
    pub as_of: Option<NaiveDate>,
    pub page: u32,
}

#[derive(Debug, Error)]
pub enum ScreenerQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait ScreenerQuerySource: Send + Sync {
    async fn screen(&self, query: ScreenerQuery) -> Result<ScreenerPage, ScreenerQueryError>;
    async fn list_sectors(&self) -> Result<Vec<ScreenerSector>, ScreenerQueryError>;
}

pub type SharedScreenerQuerySource = Arc<dyn ScreenerQuerySource + Send + Sync>;

#[derive(Debug, Error)]
pub enum ScreenerUseCaseError {
    #[error(transparent)]
    InvalidFilters(#[from] ScreenerFilterValidationError),
    #[error("page must be greater than or equal to 1")]
    InvalidPage,
    #[error(transparent)]
    Query(#[from] ScreenerQueryError),
}

#[derive(Clone)]
pub struct ScreenerUseCases {
    query: SharedScreenerQuerySource,
}

impl ScreenerUseCases {
    pub fn new(query: SharedScreenerQuerySource) -> Self {
        Self { query }
    }

    pub async fn screen(&self, query: ScreenerQuery) -> Result<ScreenerPage, ScreenerUseCaseError> {
        query.filters.validate()?;
        if query.page == 0 {
            return Err(ScreenerUseCaseError::InvalidPage);
        }

        self.query.screen(query).await.map_err(Into::into)
    }

    pub async fn list_sectors(&self) -> Result<Vec<ScreenerSector>, ScreenerUseCaseError> {
        self.query.list_sectors().await.map_err(Into::into)
    }
}
