use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecFilingDocument {
    pub form: String,
    pub report_date: NaiveDate,
    pub accession_number: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecFilingSourceError {
    #[error("SEC filing source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait SecFilingSource: Send + Sync {
    async fn fetch_periodic_reports(
        &self,
        cik: &str,
        ticker: &str,
        report_dates: &[NaiveDate],
    ) -> Result<Vec<SecFilingDocument>, SecFilingSourceError>;
}

pub type SharedSecFilingSource = Arc<dyn SecFilingSource>;
