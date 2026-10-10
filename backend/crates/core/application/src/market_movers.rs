use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketMoverDirection {
    Up,
    Down,
    Abs,
}

impl MarketMoverDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Abs => "abs",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketMoversQuery {
    pub strategy_id: Uuid,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub direction: MarketMoverDirection,
    pub min_avg_turnover: Decimal,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketMover {
    pub instrument_id: String,
    pub name: String,
    pub change_rate: Decimal,
    pub avg_turnover: Decimal,
    pub first_seen_at: Option<DateTime<FixedOffset>>,
    pub seen_via: Option<String>,
}

#[derive(Debug, Error)]
pub enum MarketMoversQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait MarketMoversQuerySource: Send + Sync {
    async fn list_movers(
        &self,
        query: MarketMoversQuery,
    ) -> Result<Vec<MarketMover>, MarketMoversQueryError>;
}

pub type SharedMarketMoversQuerySource = Arc<dyn MarketMoversQuerySource + Send + Sync>;

#[derive(Debug, Error)]
pub enum MarketMoversUseCaseError {
    #[error(transparent)]
    Query(#[from] MarketMoversQueryError),
}

#[derive(Clone)]
pub struct MarketMoversUseCases {
    query: SharedMarketMoversQuerySource,
}

impl MarketMoversUseCases {
    pub fn new(query: SharedMarketMoversQuerySource) -> Self {
        Self { query }
    }

    pub async fn list_movers(
        &self,
        query: MarketMoversQuery,
    ) -> Result<Vec<MarketMover>, MarketMoversUseCaseError> {
        self.query.list_movers(query).await.map_err(Into::into)
    }
}
