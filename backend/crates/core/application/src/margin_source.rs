use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::margin::{MarginAlertRecord, MarginInterestRecord};

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum MarginSourceError {
    #[error("margin source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait MarginSource: Send + Sync {
    /// 信用取引週末残高を、指定日の全銘柄分取得する。
    async fn fetch_margin_interest(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MarginInterestRecord>, MarginSourceError>;

    /// 日々公表信用取引残高を、指定公表日の全銘柄分取得する。
    async fn fetch_margin_alert(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<MarginAlertRecord>, MarginSourceError>;

    /// `today` 時点で取得できる日付の範囲。取得できない間は `None`。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedMarginSource = Arc<dyn MarginSource>;
