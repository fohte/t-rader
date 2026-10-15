use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMoversQuery {
    pub axis_key: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub min_avg_turnover: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupMoverStock {
    pub stock_id: String,
    pub name: String,
    pub change_rate: Decimal,
    pub avg_turnover: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupMover {
    pub group_key: String,
    pub name: String,
    pub constituent_count: i64,
    pub median_change_rate: Option<Decimal>,
    pub turnover_weighted_change_rate: Option<Decimal>,
    pub total_avg_turnover: Decimal,
    pub top_stocks: Vec<GroupMoverStock>,
}

#[derive(Debug, Error)]
pub enum GroupMoversQueryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait GroupMoversQuerySource: Send + Sync {
    async fn axis_exists(&self, axis_key: &str) -> Result<bool, GroupMoversQueryError>;

    async fn list_group_movers(
        &self,
        query: GroupMoversQuery,
    ) -> Result<Vec<GroupMover>, GroupMoversQueryError>;
}

pub type SharedGroupMoversQuerySource = Arc<dyn GroupMoversQuerySource + Send + Sync>;

#[derive(Debug, Error)]
pub enum GroupMoversUseCaseError {
    #[error("from must be on or before to")]
    InvalidDateRange,
    #[error("min_avg_turnover must be a non-negative number")]
    InvalidMinAvgTurnover,
    #[error("group axis {0} not found")]
    AxisNotFound(String),
    #[error(transparent)]
    Query(#[from] GroupMoversQueryError),
}

#[derive(Clone)]
pub struct GroupMoversUseCases {
    query: SharedGroupMoversQuerySource,
}

impl GroupMoversUseCases {
    pub fn new(query: SharedGroupMoversQuerySource) -> Self {
        Self { query }
    }

    pub async fn list_group_movers(
        &self,
        query: GroupMoversQuery,
    ) -> Result<Vec<GroupMover>, GroupMoversUseCaseError> {
        if query.from > query.to {
            return Err(GroupMoversUseCaseError::InvalidDateRange);
        }
        if query.min_avg_turnover < Decimal::ZERO {
            return Err(GroupMoversUseCaseError::InvalidMinAvgTurnover);
        }
        if !self.query.axis_exists(&query.axis_key).await? {
            return Err(GroupMoversUseCaseError::AxisNotFound(query.axis_key));
        }

        self.query
            .list_group_movers(query)
            .await
            .map_err(Into::into)
    }
}
