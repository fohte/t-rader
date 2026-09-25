use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::short_ratio::ShortRatio;
use core_domain::short_sale_report::ShortSaleReport;

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShortSellingSourceError {
    #[error("short selling source error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait ShortSellingSource: Send + Sync {
    /// 空売り残高報告を、指定公表日の全銘柄分取得する。
    async fn fetch_short_sale_reports(
        &self,
        disc_date: NaiveDate,
    ) -> Result<Vec<ShortSaleReport>, ShortSellingSourceError>;

    /// 業種別空売り比率を、指定日の全業種分取得する。
    async fn fetch_short_ratios(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<ShortRatio>, ShortSellingSourceError>;

    /// `today` 時点で取得できる日付の範囲。取得できない間は `None`。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedShortSellingSource = Arc<dyn ShortSellingSource>;
