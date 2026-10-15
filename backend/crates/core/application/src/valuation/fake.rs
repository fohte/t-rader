use std::collections::HashSet;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::valuation::Valuation;

use super::repository::{ValuationRepository, ValuationRepositoryError};

#[derive(Default)]
pub struct FakeValuationRepository {
    pub ingested_dates: Mutex<HashSet<NaiveDate>>,
    pub valuations: Mutex<Vec<Valuation>>,
    pub ingested_date_queries: Mutex<Vec<NaiveDate>>,
    pub date_range_queries: Mutex<Vec<(String, NaiveDate, NaiveDate)>>,
}

impl FakeValuationRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ValuationRepository for FakeValuationRepository {
    async fn find_ingested_dates(
        &self,
        from: NaiveDate,
    ) -> Result<Vec<NaiveDate>, ValuationRepositoryError> {
        self.ingested_date_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(from);
        Ok(self
            .ingested_dates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .copied()
            .collect())
    }

    async fn mark_ingested(&self, date: NaiveDate) -> Result<(), ValuationRepositoryError> {
        self.ingested_dates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(date);
        Ok(())
    }

    async fn upsert(&self, valuations: Vec<Valuation>) -> Result<usize, ValuationRepositoryError> {
        let count = valuations.len();
        self.valuations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend(valuations);
        Ok(count)
    }

    async fn find_by_symbol_date_range(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Valuation>, ValuationRepositoryError> {
        self.date_range_queries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((symbol.to_string(), from, to));
        Ok(self
            .valuations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|valuation| {
                (valuation.code == symbol
                    || valuation.code.chars().take(4).collect::<String>() == symbol)
                    && valuation.date >= from
                    && valuation.date <= to
            })
            .cloned()
            .collect())
    }

    async fn find_latest_by_symbol(
        &self,
        symbol: &str,
    ) -> Result<Option<Valuation>, ValuationRepositoryError> {
        Ok(self
            .valuations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|valuation| valuation.code.chars().take(4).collect::<String>() == symbol)
            .max_by_key(|valuation| valuation.date)
            .cloned())
    }
}
