use chrono::{Duration, NaiveDate};
use core_domain::short_ratio::ShortRatio;

use crate::daily_bar_source::DateRange;
use crate::short_selling_source::ShortSellingSource;
use crate::strategy_scope::StrategyScope;

use super::error::ShortRatioUseCaseError;
use super::repository::SharedShortRatioRepository;
use super::types::{ShortRatioIngestStats, ShortRatioQuery};

const SHORT_RATIO_START_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2008, 11, 5) {
    Some(date) => date,
    None => panic!("invalid short ratio start date"),
};
const CATCH_UP_LOOKBACK_DAYS: i64 = 7;

#[derive(Clone)]
pub struct ShortRatioUseCases {
    repository: SharedShortRatioRepository,
}

impl ShortRatioUseCases {
    pub fn new(repository: SharedShortRatioRepository) -> Self {
        Self { repository }
    }

    pub async fn ingest(
        &self,
        source: &dyn ShortSellingSource,
        today: NaiveDate,
    ) -> Result<ShortRatioIngestStats, ShortRatioUseCaseError> {
        let mut stats = ShortRatioIngestStats::default();
        let Some(range) = source.fetchable_range(today) else {
            tracing::debug!("取得できないため取り込みをスキップ");
            return Ok(stats);
        };

        let floor = range.from.max(SHORT_RATIO_START_DATE);
        let latest = self.repository.find_latest_date().await?;
        let from = latest
            .map(|date| (date - Duration::days(CATCH_UP_LOOKBACK_DAYS)).max(floor))
            .unwrap_or(floor);

        self.fetch_and_save(source, DateRange { from, to: today }, &mut stats)
            .await?;
        Ok(stats)
    }

    async fn fetch_and_save(
        &self,
        source: &dyn ShortSellingSource,
        range: DateRange,
        stats: &mut ShortRatioIngestStats,
    ) -> Result<(), ShortRatioUseCaseError> {
        let mut date = range.from;
        while date <= range.to {
            match source.fetch_short_ratios(date).await {
                Ok(ratios) => {
                    stats.days_fetched += 1;
                    if !ratios.is_empty() {
                        stats.rows_upserted += ratios.len();
                        self.repository.upsert(ratios).await?;
                    }
                }
                Err(error) => {
                    tracing::warn!(%date, %error, "取得に失敗、サイクルを打ち切り");
                    break;
                }
            }
            date += Duration::days(1);
        }
        Ok(())
    }

    pub async fn read(
        &self,
        _scope: StrategyScope,
        query: ShortRatioQuery,
    ) -> Result<Vec<ShortRatio>, ShortRatioUseCaseError> {
        self.repository.read(query).await.map_err(Into::into)
    }
}
