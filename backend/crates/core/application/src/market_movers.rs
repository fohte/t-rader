use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate};
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::strategy_scope::StrategyScope;

const MAX_MARKET_MOVERS_LIMIT: u32 = 100;

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
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub direction: MarketMoverDirection,
    pub min_avg_turnover: Decimal,
    pub limit: u32,
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
        strategy_id: Uuid,
        query: MarketMoversQuery,
    ) -> Result<Vec<MarketMover>, MarketMoversQueryError>;
}

pub type SharedMarketMoversQuerySource = Arc<dyn MarketMoversQuerySource + Send + Sync>;

#[derive(Debug, Error)]
pub enum MarketMoversUseCaseError {
    #[error("from must be on or before to")]
    InvalidDateRange,
    #[error("limit must be between 1 and 100")]
    InvalidLimit,
    #[error("min_avg_turnover must be a non-negative number")]
    InvalidMinAvgTurnover,
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
        scope: StrategyScope,
        query: MarketMoversQuery,
    ) -> Result<Vec<MarketMover>, MarketMoversUseCaseError> {
        if query.from > query.to {
            return Err(MarketMoversUseCaseError::InvalidDateRange);
        }
        if query.limit == 0 || query.limit > MAX_MARKET_MOVERS_LIMIT {
            return Err(MarketMoversUseCaseError::InvalidLimit);
        }
        if query.min_avg_turnover < Decimal::ZERO {
            return Err(MarketMoversUseCaseError::InvalidMinAvgTurnover);
        }

        self.query
            .list_movers(scope.id(), query)
            .await
            .map_err(Into::into)
    }
}
