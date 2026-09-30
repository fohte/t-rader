use chrono::{Duration, NaiveDate};
use core_domain::short_sale_report::ShortSaleReport;

use crate::daily_bar_source::DateRange;
use crate::short_selling_source::ShortSellingSource;
use crate::strategy_scope::StrategyScope;

use super::error::ShortSaleReportUseCaseError;
use super::repository::SharedShortSaleReportRepository;
use super::types::{ShortSaleReportIngestStats, ShortSaleReportQuery};

const SHORT_SALE_REPORT_START_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2013, 11, 7) {
    Some(date) => date,
    None => panic!("invalid short sale report start date"),
};
const CATCH_UP_LOOKBACK_DAYS: i64 = 7;

#[derive(Clone)]
pub struct ShortSaleReportUseCases {
    repository: SharedShortSaleReportRepository,
}

impl ShortSaleReportUseCases {
    pub fn new(repository: SharedShortSaleReportRepository) -> Self {
        Self { repository }
    }

    pub async fn ingest(
        &self,
        source: &dyn ShortSellingSource,
        today: NaiveDate,
    ) -> Result<ShortSaleReportIngestStats, ShortSaleReportUseCaseError> {
        let mut stats = ShortSaleReportIngestStats::default();
        let Some(range) = source.fetchable_range(today) else {
            tracing::debug!("取得できないため取り込みをスキップ");
            return Ok(stats);
        };

        let floor = range.from.max(SHORT_SALE_REPORT_START_DATE);
        let latest = self.repository.find_latest_disc_date().await?;
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
        stats: &mut ShortSaleReportIngestStats,
    ) -> Result<(), ShortSaleReportUseCaseError> {
        let mut date = range.from;
        while date <= range.to {
            match source.fetch_short_sale_reports(date).await {
                Ok(reports) => {
                    stats.days_fetched += 1;
                    if !reports.is_empty() {
                        stats.rows_upserted += reports.len();
                        self.repository.upsert(reports).await?;
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
        query: ShortSaleReportQuery,
    ) -> Result<Vec<ShortSaleReport>, ShortSaleReportUseCaseError> {
        self.repository.read(query).await.map_err(Into::into)
    }
}
