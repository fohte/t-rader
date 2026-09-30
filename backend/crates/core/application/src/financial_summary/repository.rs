use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::financial_summary::FinancialSummary;
use thiserror::Error;

use crate::persistence::PersistenceError;
use crate::unit_of_work::UnitOfWorkTransaction;

#[derive(Debug, Error)]
pub enum FinancialSummaryRepositoryError {
    #[error(transparent)]
    Database(#[from] PersistenceError),
    #[error("transaction has an unexpected type")]
    InvalidTransaction,
}

#[async_trait]
pub trait FinancialSummaryRepository: Send + Sync {
    async fn latest_disclosure_date(
        &self,
    ) -> Result<Option<NaiveDate>, FinancialSummaryRepositoryError>;

    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        summaries: Vec<FinancialSummary>,
    ) -> Result<usize, FinancialSummaryRepositoryError>;

    async fn find_for_symbol(
        &self,
        symbol: &str,
        limit: u64,
    ) -> Result<Vec<FinancialSummary>, FinancialSummaryRepositoryError>;
}

pub type SharedFinancialSummaryRepository =
    std::sync::Arc<dyn FinancialSummaryRepository + Send + Sync>;
