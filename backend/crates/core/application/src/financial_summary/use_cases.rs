use chrono::{Duration, NaiveDate};
use core_domain::business_day::latest_business_day;
use core_domain::financial_summary::FinancialSummary;
use tracing::{debug, warn};

use crate::daily_bar_source::DateRange;
use crate::financial_summary::error::FinancialSummaryUseCaseError;
use crate::financial_summary::repository::SharedFinancialSummaryRepository;
use crate::financial_summary_source::FinancialSummarySource;
use crate::strategy_scope::StrategyScope;
use crate::unit_of_work::SharedUnitOfWork;

const LOOKBACK_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FinancialSummaryIngestStats {
    pub days_attempted: usize,
    pub upserted: usize,
}

#[derive(Clone)]
pub struct FinancialSummaryUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedFinancialSummaryRepository,
}

impl FinancialSummaryUseCases {
    pub fn new(
        unit_of_work: SharedUnitOfWork,
        repository: SharedFinancialSummaryRepository,
    ) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    /// 取得可能範囲を調べ、財務情報を日付ごとに取得して保存する。
    pub async fn run_ingest_cycle(
        &self,
        source: &dyn FinancialSummarySource,
        today: NaiveDate,
    ) -> Result<FinancialSummaryIngestStats, FinancialSummaryUseCaseError> {
        let Some(range) = source.fetchable_range(today) else {
            debug!("財務情報を取得できないため取り込みをスキップします");
            return Ok(FinancialSummaryIngestStats::default());
        };

        let latest_stored = self.repository.latest_disclosure_date().await?;
        let Some((from, to)) = fetch_range(range, latest_stored) else {
            debug!("財務情報の取得可能範囲がデータ提供開始日に届かないためスキップします");
            return Ok(FinancialSummaryIngestStats::default());
        };

        let mut stats = FinancialSummaryIngestStats::default();
        let mut date = from;
        while date <= to {
            if latest_business_day(date) == date {
                stats.days_attempted += 1;
                match source.fetch_financial_summaries_by_date(date).await {
                    Ok(items) => match self.upsert_day(items).await {
                        Ok(count) => stats.upserted += count,
                        Err(error) => {
                            warn!(%date, %error, "財務情報の格納に失敗、この日をスキップします");
                        }
                    },
                    Err(error) => {
                        warn!(%date, %error, "財務情報の取得に失敗、この日をスキップします");
                    }
                }
            }
            date += Duration::days(1);
        }

        Ok(stats)
    }

    /// 戦略に属さない財務情報を銘柄コードで検索する。
    pub async fn find_for_symbol(
        &self,
        _scope: StrategyScope,
        symbol: &str,
        limit: u64,
    ) -> Result<Vec<FinancialSummary>, FinancialSummaryUseCaseError> {
        self.repository
            .find_for_symbol(symbol, limit)
            .await
            .map_err(Into::into)
    }

    async fn upsert_day(
        &self,
        summaries: Vec<FinancialSummary>,
    ) -> Result<usize, FinancialSummaryUseCaseError> {
        if summaries.is_empty() {
            return Ok(0);
        }

        let transaction = self.unit_of_work.begin().await?;
        let upserted = self.repository.upsert(&transaction, summaries).await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(upserted)
    }
}

fn fetch_range(
    range: DateRange,
    latest_stored: Option<NaiveDate>,
) -> Option<(NaiveDate, NaiveDate)> {
    let range_from = range.from.max(provision_start_date());
    if range_from > range.to {
        return None;
    }

    let from = latest_stored
        .map(|latest| (latest - Duration::days(LOOKBACK_DAYS)).max(range_from))
        .unwrap_or(range_from);
    Some((from, range.to))
}

fn provision_start_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2008, 7, 7).unwrap_or_default()
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::financial_summary::FinancialSummary;
    use tokio::sync::Mutex;

    use crate::daily_bar_source::DateRange;
    use crate::financial_summary::{
        FinancialSummaryRepository, FinancialSummaryRepositoryError, FinancialSummaryUseCases,
    };
    use crate::financial_summary_source::{FinancialSummarySource, FinancialSummarySourceError};
    use crate::strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError};
    use crate::unit_of_work::{
        FakeTransaction, FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction,
    };

    #[derive(Default)]
    struct FakeRepository {
        latest: Option<NaiveDate>,
        upserts: Mutex<Vec<(uuid::Uuid, Vec<FinancialSummary>)>>,
        queries: Mutex<Vec<(String, u64)>>,
    }

    #[async_trait]
    impl FinancialSummaryRepository for FakeRepository {
        async fn latest_disclosure_date(
            &self,
        ) -> Result<Option<NaiveDate>, FinancialSummaryRepositoryError> {
            Ok(self.latest)
        }

        async fn upsert(
            &self,
            transaction: &UnitOfWorkTransaction,
            summaries: Vec<FinancialSummary>,
        ) -> Result<usize, FinancialSummaryRepositoryError> {
            let id = transaction
                .downcast_ref::<FakeTransaction>()
                .map(|transaction| transaction.id)
                .ok_or(FinancialSummaryRepositoryError::InvalidTransaction)?;
            let count = summaries.len();
            self.upserts.lock().await.push((id, summaries));
            Ok(count)
        }

        async fn find_for_symbol(
            &self,
            symbol: &str,
            limit: u64,
        ) -> Result<Vec<FinancialSummary>, FinancialSummaryRepositoryError> {
            self.queries.lock().await.push((symbol.to_owned(), limit));
            Ok(Vec::new())
        }
    }

    struct FakeSource {
        range: Option<DateRange>,
        fetched_dates: Mutex<Vec<NaiveDate>>,
        summaries: Vec<FinancialSummary>,
    }

    #[async_trait]
    impl FinancialSummarySource for FakeSource {
        async fn fetch_financial_summaries_by_date(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<FinancialSummary>, FinancialSummarySourceError> {
            self.fetched_dates.lock().await.push(date);
            Ok(self.summaries.clone())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            self.range.clone()
        }
    }

    #[expect(
        clippy::expect_used,
        reason = "固定のテスト日付が無効なら即時に失敗させるため"
    )]
    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    fn summary(disclosure_date: NaiveDate) -> FinancialSummary {
        FinancialSummary {
            code: "ZZ999".to_owned(),
            disclosure_no: "1".to_owned(),
            disclosure_date,
            report_group_key: "sample-report-group".to_owned(),
            document_type: None,
            current_period_type: None,
            current_period_start: None,
            current_period_end: None,
            current_fiscal_year_start: None,
            current_fiscal_year_end: None,
            sales: None,
            operating_profit: None,
            ordinary_profit: None,
            net_profit: None,
            eps: None,
            bps: None,
            total_assets: None,
            equity: None,
            equity_to_asset_ratio: None,
            roe: None,
            cash_flow_operating: None,
            cash_flow_investing: None,
            cash_flow_financing: None,
            cash_and_equivalents: None,
            dividend_annual: None,
            dividend_annual_forecast: None,
            dividend_annual_forecast_next: None,
            forecast_sales: None,
            forecast_operating_profit: None,
            forecast_ordinary_profit: None,
            forecast_net_profit: None,
            forecast_eps: None,
            next_forecast_sales: None,
            next_forecast_operating_profit: None,
            next_forecast_ordinary_profit: None,
            next_forecast_net_profit: None,
            next_forecast_eps: None,
        }
    }

    fn build_use_cases(
        repository: Arc<FakeRepository>,
        unit_of_work: SharedUnitOfWork,
    ) -> FinancialSummaryUseCases {
        FinancialSummaryUseCases::new(unit_of_work, repository)
    }

    #[tokio::test]
    async fn ingest_fetches_business_days_and_commits_each_day() {
        let from = date(2042, 2, 3);
        let to = date(2042, 2, 5);
        let repository = Arc::new(FakeRepository::default());
        let source = Arc::new(FakeSource {
            range: Some(DateRange { from, to }),
            fetched_dates: Mutex::new(Vec::new()),
            summaries: vec![summary(from)],
        });
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let use_cases = build_use_cases(repository.clone(), unit_of_work.clone());

        let stats = use_cases
            .run_ingest_cycle(source.as_ref(), to)
            .await
            .expect("ingest succeeds");
        let fetched_dates = source.fetched_dates.lock().await.clone();
        let upserts = repository.upserts.lock().await.clone();
        let begun = unit_of_work.begun.lock().await.clone();
        let committed = unit_of_work.committed.lock().await.clone();

        assert_eq!(
            (
                stats,
                fetched_dates,
                upserts.len(),
                begun.len(),
                committed == begun,
                upserts.iter().map(|(id, _)| *id).collect::<Vec<_>>() == begun,
            ),
            (
                super::FinancialSummaryIngestStats {
                    days_attempted: 3,
                    upserted: 3,
                },
                vec![from, date(2042, 2, 4), to],
                3,
                3,
                true,
                true,
            ),
        );
    }

    #[tokio::test]
    async fn read_for_symbol_uses_repository_without_a_transaction() {
        let repository = Arc::new(FakeRepository::default());
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let use_cases = build_use_cases(repository.clone(), unit_of_work.clone());
        let strategy_id = uuid::Uuid::new_v4();
        struct ExistingStrategy(uuid::Uuid);
        #[async_trait]
        impl StrategyScopeSource for ExistingStrategy {
            async fn existing_ids(
                &self,
                ids: &[uuid::Uuid],
            ) -> Result<std::collections::HashSet<uuid::Uuid>, StrategyScopeSourceError>
            {
                Ok(ids.iter().copied().filter(|id| *id == self.0).collect())
            }
        }
        let scope = StrategyScope::verify(strategy_id, &ExistingStrategy(strategy_id))
            .await
            .expect("strategy exists");

        use_cases
            .find_for_symbol(scope, "ZZ99", 25)
            .await
            .expect("query succeeds");

        assert_eq!(
            (
                repository.queries.lock().await.clone(),
                unit_of_work.begun.lock().await.is_empty(),
            ),
            (vec![("ZZ99".to_owned(), 25)], true),
        );
    }
}
