use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::FinancialSummary;

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FinancialSummarySourceError {
    #[error("financial summary source error: {0}")]
    Failed(String),
}

/// 財務情報を開示日ごとに取得する。
#[async_trait]
pub trait FinancialSummarySource: Send + Sync {
    async fn fetch_financial_summaries_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<FinancialSummary>, FinancialSummarySourceError>;

    /// `today` 時点で取得できる日付の範囲。取得できない間は `None`。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedFinancialSummarySource = Arc<dyn FinancialSummarySource>;
