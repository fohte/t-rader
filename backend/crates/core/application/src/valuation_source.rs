use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::valuation::Valuation;

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ValuationSourceError {
    #[error("valuation source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait ValuationSource: Send + Sync {
    /// 指定日の全銘柄分のバリュエーション指標を取得する。
    async fn fetch_valuations_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<Valuation>, ValuationSourceError>;

    /// `today` 時点で取得できる日付の範囲。取得できない間は `None`。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedValuationSource = Arc<dyn ValuationSource>;
