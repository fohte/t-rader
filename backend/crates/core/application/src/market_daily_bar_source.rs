use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use core_domain::bar::Bar;

use crate::DateRange;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum MarketDailyBarSourceError {
    #[error("market daily bar source error: {0}")]
    Failed(String),
}

/// 日付を指定して全上場銘柄の日足をまとめて取得する。銘柄ごとの取得は `DailyBarSource`。
#[async_trait]
pub trait MarketDailyBarSource: Send + Sync {
    async fn fetch_daily_bars_by_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<Bar>, MarketDailyBarSourceError>;

    /// `today` 時点で取得できる日付の範囲。取得できない間は `None`。
    fn fetchable_range(&self, today: NaiveDate) -> Option<DateRange>;
}

pub type SharedMarketDailyBarSource = Arc<dyn MarketDailyBarSource>;
