use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::valuation::Valuation;
use thiserror::Error;

use crate::persistence::PersistenceError;

#[derive(Debug, Error)]
pub enum ValuationRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
}

#[async_trait]
pub trait ValuationRepository: Send + Sync {
    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<Vec<NaiveDate>, ValuationRepositoryError>;

    async fn mark_ingested(&self, date: NaiveDate) -> Result<(), ValuationRepositoryError>;

    async fn upsert(&self, valuations: Vec<Valuation>) -> Result<usize, ValuationRepositoryError>;

    async fn find_by_symbol_date_range(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Valuation>, ValuationRepositoryError>;

    async fn find_latest_by_symbol(
        &self,
        symbol: &str,
    ) -> Result<Option<Valuation>, ValuationRepositoryError> {
        Ok(self
            .find_by_symbol_date_range(symbol, NaiveDate::MIN, NaiveDate::MAX)
            .await?
            .into_iter()
            .max_by_key(|valuation| valuation.date))
    }
}

pub type SharedValuationRepository = Arc<dyn ValuationRepository>;
