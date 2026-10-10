use std::collections::HashSet;

use chrono::{Duration as ChronoDuration, NaiveDate};
use core_domain::business_day::latest_business_day;
use core_domain::valuation::Valuation;
use serde::Serialize;

use crate::bars::{DailyBarAdjustmentFactor, SharedBarsRepository};
use crate::daily_bar_source::DateRange;
use crate::strategy_scope::StrategyScope;
use crate::valuation_source::ValuationSource;

use super::error::ValuationUseCaseError;
use super::repository::SharedValuationRepository;

const TARGET_BUSINESS_DAYS: usize = 400;
const REFETCH_WINDOW_BUSINESS_DAYS: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct IngestStats {
    pub days_attempted: usize,
    pub rows_upserted: usize,
}

#[derive(Clone)]
pub struct ValuationUseCases {
    repository: SharedValuationRepository,
    bars_repository: SharedBarsRepository,
}

impl ValuationUseCases {
    pub fn new(
        repository: SharedValuationRepository,
        bars_repository: SharedBarsRepository,
    ) -> Self {
        Self {
            repository,
            bars_repository,
        }
    }

    pub async fn run_ingest_cycle(
        &self,
        source: &dyn ValuationSource,
        today: NaiveDate,
    ) -> Result<IngestStats, ValuationUseCaseError> {
        let Some(range) = source.fetchable_range(today) else {
            tracing::debug!("valuation を取得できないため取り込みをスキップします");
            return Ok(IngestStats::default());
        };

        let business_days = recent_business_days_in_range(&range, today);
        let Some(&earliest) = business_days.first() else {
            return Ok(IngestStats::default());
        };

        let ingested = self
            .repository
            .find_ingested_dates(earliest)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
        let targets = target_dates(&business_days, &ingested);
        let mut stats = IngestStats::default();

        for date in targets {
            stats.days_attempted += 1;
            match source.fetch_valuations_by_date(date).await {
                Ok(items) if items.is_empty() => {
                    tracing::debug!(%date, "この日の valuation はまだ公開されていません");
                }
                Ok(items) => match self.repository.upsert(items).await {
                    Ok(row_count) => {
                        stats.rows_upserted += row_count;
                        if let Err(error) = self.repository.mark_ingested(date).await {
                            tracing::warn!(%date, %error, "valuation の取り込み日記録に失敗、この日を再試行します");
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%date, %error, "valuation の格納に失敗、この日をスキップします");
                    }
                },
                Err(error) => {
                    tracing::warn!(%date, %error, "valuation の取得に失敗、この日をスキップします");
                }
            }
        }

        Ok(stats)
    }

    pub async fn find_for_symbol(
        &self,
        _scope: StrategyScope,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Valuation>, ValuationUseCaseError> {
        let mut valuations = self
            .repository
            .find_by_symbol_date_range(symbol, from, to)
            .await
            .map_err(ValuationUseCaseError::from)?;
        let adjustment_factors = self
            .bars_repository
            .find_daily_adjustment_factors_from(symbol, from)
            .await?;

        apply_daily_bar_adjustment_factors(&mut valuations, &adjustment_factors);
        Ok(valuations)
    }
}

fn apply_daily_bar_adjustment_factors(
    valuations: &mut [Valuation],
    adjustment_factors: &[DailyBarAdjustmentFactor],
) {
    for valuation in valuations {
        let factor = adjustment_factors
            .iter()
            .filter(|adjustment| adjustment.date > valuation.date)
            .map(|adjustment| adjustment.factor)
            .product::<rust_decimal::Decimal>();

        valuation.eps = valuation.eps.map(|value| value * factor);
        valuation.fwd_eps = valuation.fwd_eps.map(|value| value * factor);
        valuation.bps = valuation.bps.map(|value| value * factor);
    }
}

fn recent_business_days(to: NaiveDate, count: usize) -> Vec<NaiveDate> {
    let mut days = Vec::with_capacity(count);
    let mut date = to;
    while days.len() < count {
        if latest_business_day(date) == date {
            days.push(date);
        }
        date -= ChronoDuration::days(1);
    }
    days.reverse();
    days
}

fn target_dates(business_days: &[NaiveDate], ingested: &HashSet<NaiveDate>) -> Vec<NaiveDate> {
    let refetch_from_index = business_days
        .len()
        .saturating_sub(REFETCH_WINDOW_BUSINESS_DAYS);
    business_days
        .iter()
        .enumerate()
        .filter(|(index, date)| *index >= refetch_from_index || !ingested.contains(date))
        .map(|(_, date)| *date)
        .collect()
}

fn recent_business_days_in_range(range: &DateRange, today: NaiveDate) -> Vec<NaiveDate> {
    recent_business_days(
        latest_business_day(range.to.min(today)),
        TARGET_BUSINESS_DAYS,
    )
    .into_iter()
    .filter(|date| *date >= range.from)
    .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use chrono::NaiveDate;
    use core_domain::valuation::Valuation;
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use crate::daily_bar_source::DateRange;
    use crate::strategy_scope::{StrategyScope, StrategyScopeSource, StrategyScopeSourceError};
    use crate::valuation_source::{ValuationSource, ValuationSourceError};

    use super::{IngestStats, ValuationUseCases};
    use crate::bars::{DailyBarAdjustmentFactor, FakeBarsRepository};
    use crate::valuation::{ValuationRepository, ValuationRepositoryError};

    #[derive(Default)]
    struct FakeValuationRepository {
        ingested_dates: Mutex<HashSet<NaiveDate>>,
        valuations: Mutex<Vec<Valuation>>,
        ingested_date_queries: Mutex<Vec<NaiveDate>>,
        read_query: Mutex<Option<(String, NaiveDate, NaiveDate)>>,
    }

    use std::collections::HashSet;

    #[async_trait]
    impl ValuationRepository for FakeValuationRepository {
        async fn find_ingested_dates(
            &self,
            from: NaiveDate,
        ) -> Result<Vec<NaiveDate>, ValuationRepositoryError> {
            self.ingested_date_queries.lock().expect("lock").push(from);
            Ok(self
                .ingested_dates
                .lock()
                .expect("lock")
                .iter()
                .copied()
                .collect())
        }

        async fn mark_ingested(&self, date: NaiveDate) -> Result<(), ValuationRepositoryError> {
            self.ingested_dates.lock().expect("lock").insert(date);
            Ok(())
        }

        async fn upsert(
            &self,
            valuations: Vec<Valuation>,
        ) -> Result<usize, ValuationRepositoryError> {
            let count = valuations.len();
            self.valuations.lock().expect("lock").extend(valuations);
            Ok(count)
        }

        async fn find_by_symbol_date_range(
            &self,
            symbol: &str,
            from: NaiveDate,
            to: NaiveDate,
        ) -> Result<Vec<Valuation>, ValuationRepositoryError> {
            *self.read_query.lock().expect("lock") = Some((symbol.to_string(), from, to));
            Ok(self.valuations.lock().expect("lock").clone())
        }
    }

    struct FakeValuationSource {
        range: Option<DateRange>,
        valuations: Vec<Valuation>,
        requested_dates: Mutex<Vec<NaiveDate>>,
    }

    #[async_trait]
    impl ValuationSource for FakeValuationSource {
        async fn fetch_valuations_by_date(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<Valuation>, ValuationSourceError> {
            self.requested_dates.lock().expect("lock").push(date);
            let mut valuations = self.valuations.clone();
            for valuation in &mut valuations {
                valuation.date = date;
            }
            Ok(valuations)
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            self.range.clone()
        }
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    #[fixture]
    fn repository() -> Arc<FakeValuationRepository> {
        Arc::new(FakeValuationRepository::default())
    }

    #[fixture]
    fn bars_repository() -> Arc<FakeBarsRepository> {
        Arc::new(FakeBarsRepository::new())
    }

    #[fixture]
    fn business_days() -> Vec<NaiveDate> {
        super::recent_business_days(date(2099, 1, 9), 10)
    }

    fn sample_valuation(date: NaiveDate) -> Valuation {
        Valuation {
            code: "ZZZZ0".to_string(),
            date,
            eps: Some(Decimal::new(125, 1)),
            fwd_eps: None,
            bps: None,
            roe: None,
            fwd_roe: None,
            per: None,
            fwd_per: None,
            pbr: None,
            mkt_cap: None,
        }
    }

    struct ExistingStrategy;

    #[async_trait]
    impl StrategyScopeSource for ExistingStrategy {
        async fn existing_ids(
            &self,
            ids: &[Uuid],
        ) -> Result<HashSet<Uuid>, StrategyScopeSourceError> {
            Ok(ids.iter().copied().collect())
        }
    }

    async fn strategy_scope() -> StrategyScope {
        StrategyScope::verify(Uuid::nil(), &ExistingStrategy)
            .await
            .expect("scope succeeds")
    }

    #[rstest]
    #[tokio::test]
    async fn ingests_missing_dates_and_refetches_recent_dates(
        repository: Arc<FakeValuationRepository>,
        business_days: Vec<NaiveDate>,
        bars_repository: Arc<FakeBarsRepository>,
    ) {
        let today = *business_days.last().expect("business days exist");
        let targets = vec![
            business_days[0],
            business_days[3],
            business_days[4],
            business_days[5],
            business_days[6],
            business_days[7],
            business_days[8],
            business_days[9],
        ];
        *repository.ingested_dates.lock().expect("lock") =
            HashSet::from([business_days[1], business_days[2], business_days[8]]);
        let source = Arc::new(FakeValuationSource {
            range: Some(DateRange {
                from: business_days[0],
                to: today,
            }),
            valuations: vec![sample_valuation(today)],
            requested_dates: Mutex::new(Vec::new()),
        });
        let use_cases = ValuationUseCases::new(repository.clone(), bars_repository);

        let stats = use_cases
            .run_ingest_cycle(source.as_ref(), today)
            .await
            .expect("cycle succeeds");
        let observed = (
            stats,
            repository.valuations.lock().expect("lock").clone(),
            repository.ingested_dates.lock().expect("lock").clone(),
            repository
                .ingested_date_queries
                .lock()
                .expect("lock")
                .clone(),
            source.requested_dates.lock().expect("lock").clone(),
        );

        assert_eq!(
            observed,
            (
                IngestStats {
                    days_attempted: 8,
                    rows_upserted: 8,
                },
                targets.iter().map(|date| sample_valuation(*date)).collect(),
                business_days.iter().copied().collect(),
                vec![business_days[0]],
                targets,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn skips_ingestion_when_source_has_no_fetchable_range(
        repository: Arc<FakeValuationRepository>,
        bars_repository: Arc<FakeBarsRepository>,
    ) {
        let source = FakeValuationSource {
            range: None,
            valuations: Vec::new(),
            requested_dates: Mutex::new(Vec::new()),
        };
        let use_cases = ValuationUseCases::new(repository.clone(), bars_repository);

        let result = use_cases
            .run_ingest_cycle(&source, date(2099, 1, 9))
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (
                result,
                repository
                    .ingested_date_queries
                    .lock()
                    .expect("lock")
                    .clone(),
                repository.valuations.lock().expect("lock").clone(),
                repository.ingested_dates.lock().expect("lock").clone(),
                source.requested_dates.lock().expect("lock").clone(),
            ),
            (
                IngestStats::default(),
                Vec::new(),
                Vec::new(),
                HashSet::new(),
                Vec::new(),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn does_not_mark_dates_when_source_returns_no_valuations(
        repository: Arc<FakeValuationRepository>,
        bars_repository: Arc<FakeBarsRepository>,
    ) {
        let today = date(2099, 1, 9);
        let expected_date = core_domain::business_day::latest_business_day(today);
        let source = FakeValuationSource {
            range: Some(DateRange {
                from: expected_date,
                to: expected_date,
            }),
            valuations: Vec::new(),
            requested_dates: Mutex::new(Vec::new()),
        };
        let use_cases = ValuationUseCases::new(repository.clone(), bars_repository);

        let result = use_cases
            .run_ingest_cycle(&source, today)
            .await
            .expect("cycle succeeds");

        assert_eq!(
            (
                result,
                repository.ingested_dates.lock().expect("lock").clone(),
                repository.valuations.lock().expect("lock").clone(),
                source.requested_dates.lock().expect("lock").clone(),
            ),
            (
                IngestStats {
                    days_attempted: 1,
                    rows_upserted: 0,
                },
                HashSet::new(),
                Vec::new(),
                vec![expected_date],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn find_for_symbol_passes_query_to_repositories(
        repository: Arc<FakeValuationRepository>,
        bars_repository: Arc<FakeBarsRepository>,
    ) {
        let from = date(2099, 1, 2);
        let to = date(2099, 1, 10);
        let expected = vec![sample_valuation(date(2099, 1, 3))];
        *repository.valuations.lock().expect("lock") = expected.clone();
        let use_cases = ValuationUseCases::new(repository.clone(), bars_repository.clone());

        let result = use_cases
            .find_for_symbol(strategy_scope().await, "ZZZZ", from, to)
            .await
            .expect("read succeeds");
        let valuation_query = repository.read_query.lock().expect("lock").clone();
        let adjustment_factor_queries = bars_repository
            .daily_adjustment_factor_queries
            .lock()
            .await
            .clone();

        assert_eq!(
            (result, (valuation_query, adjustment_factor_queries)),
            (
                expected,
                (
                    Some(("ZZZZ".to_string(), from, to)),
                    vec![("ZZZZ".to_string(), from)],
                ),
            ),
        );
    }

    #[tokio::test]
    async fn adjusts_per_share_indicators_to_the_latest_split_basis() {
        let repository = Arc::new(FakeValuationRepository::default());
        let bars_repository = Arc::new(FakeBarsRepository::new());
        let first_date = date(2099, 1, 2);
        let split_date = date(2099, 1, 5);
        let after_split_date = date(2099, 1, 6);
        *repository.valuations.lock().expect("lock") = vec![
            Valuation {
                code: "ZZZZ0".to_string(),
                date: first_date,
                eps: Some(Decimal::new(400, 0)),
                fwd_eps: Some(Decimal::new(500, 0)),
                bps: Some(Decimal::new(800, 0)),
                roe: Some(Decimal::new(8, 2)),
                fwd_roe: Some(Decimal::new(10, 2)),
                per: Some(Decimal::new(152, 1)),
                fwd_per: Some(Decimal::new(123, 1)),
                pbr: Some(Decimal::new(11, 1)),
                mkt_cap: Some(Decimal::new(250_000, 0)),
            },
            Valuation {
                code: "ZZZZ0".to_string(),
                date: split_date,
                eps: Some(Decimal::new(200, 0)),
                fwd_eps: Some(Decimal::new(250, 0)),
                bps: Some(Decimal::new(400, 0)),
                roe: Some(Decimal::new(8, 2)),
                fwd_roe: Some(Decimal::new(10, 2)),
                per: Some(Decimal::new(152, 1)),
                fwd_per: Some(Decimal::new(123, 1)),
                pbr: Some(Decimal::new(11, 1)),
                mkt_cap: Some(Decimal::new(250_000, 0)),
            },
            Valuation {
                code: "ZZZZ0".to_string(),
                date: after_split_date,
                eps: Some(Decimal::new(200, 0)),
                fwd_eps: Some(Decimal::new(250, 0)),
                bps: Some(Decimal::new(400, 0)),
                roe: Some(Decimal::new(8, 2)),
                fwd_roe: Some(Decimal::new(10, 2)),
                per: Some(Decimal::new(152, 1)),
                fwd_per: Some(Decimal::new(123, 1)),
                pbr: Some(Decimal::new(11, 1)),
                mkt_cap: Some(Decimal::new(250_000, 0)),
            },
        ];
        bars_repository
            .seed_daily_adjustment_factors(vec![
                DailyBarAdjustmentFactor {
                    date: first_date,
                    factor: Decimal::new(4, 1),
                },
                DailyBarAdjustmentFactor {
                    date: split_date,
                    factor: Decimal::new(5, 1),
                },
                DailyBarAdjustmentFactor {
                    date: date(2099, 2, 3),
                    factor: Decimal::new(25, 2),
                },
            ])
            .await;
        let use_cases = ValuationUseCases::new(repository, bars_repository);

        let result = use_cases
            .find_for_symbol(strategy_scope().await, "ZZZZ", first_date, after_split_date)
            .await
            .expect("read succeeds");

        assert_eq!(
            result,
            vec![
                Valuation {
                    code: "ZZZZ0".to_string(),
                    date: first_date,
                    eps: Some(Decimal::new(50, 0)),
                    fwd_eps: Some(Decimal::new(625, 1)),
                    bps: Some(Decimal::new(100, 0)),
                    roe: Some(Decimal::new(8, 2)),
                    fwd_roe: Some(Decimal::new(10, 2)),
                    per: Some(Decimal::new(152, 1)),
                    fwd_per: Some(Decimal::new(123, 1)),
                    pbr: Some(Decimal::new(11, 1)),
                    mkt_cap: Some(Decimal::new(250_000, 0)),
                },
                Valuation {
                    code: "ZZZZ0".to_string(),
                    date: split_date,
                    eps: Some(Decimal::new(50, 0)),
                    fwd_eps: Some(Decimal::new(625, 1)),
                    bps: Some(Decimal::new(100, 0)),
                    roe: Some(Decimal::new(8, 2)),
                    fwd_roe: Some(Decimal::new(10, 2)),
                    per: Some(Decimal::new(152, 1)),
                    fwd_per: Some(Decimal::new(123, 1)),
                    pbr: Some(Decimal::new(11, 1)),
                    mkt_cap: Some(Decimal::new(250_000, 0)),
                },
                Valuation {
                    code: "ZZZZ0".to_string(),
                    date: after_split_date,
                    eps: Some(Decimal::new(50, 0)),
                    fwd_eps: Some(Decimal::new(625, 1)),
                    bps: Some(Decimal::new(100, 0)),
                    roe: Some(Decimal::new(8, 2)),
                    fwd_roe: Some(Decimal::new(10, 2)),
                    per: Some(Decimal::new(152, 1)),
                    fwd_per: Some(Decimal::new(123, 1)),
                    pbr: Some(Decimal::new(11, 1)),
                    mkt_cap: Some(Decimal::new(250_000, 0)),
                },
            ],
        );
    }
}
