use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use core_domain::business_day::latest_business_day;
use rust_decimal::Decimal;

use crate::daily_bar_source::DailyBarSource;

use super::error::BarsUseCaseError;
use super::types::LatestPrices;
use super::use_cases::BarsUseCases;

impl BarsUseCases {
    /// 銘柄ごとの直近終値を共通の観測日でそろえて返す。
    pub async fn fetch_latest_prices(
        &self,
        source: Option<&dyn DailyBarSource>,
        instrument_ids: &[String],
    ) -> LatestPrices {
        let today = Utc::now().date_naive();
        let fetchable_ceiling = source.map(|source| {
            latest_business_day(latest_fetchable_date(today, source.known_fetchable_range()))
        });
        let mut bars: HashMap<String, (NaiveDate, Decimal)> = HashMap::new();

        for instrument_id in instrument_ids {
            let mut bar = match self.repository.find_latest_bar(instrument_id, "1d").await {
                Ok(bar) => bar,
                Err(error) => {
                    tracing::warn!(instrument_id, error = %error, "直近終値の取得に失敗");
                    None
                }
            };

            let is_stale = fetchable_ceiling.is_some_and(|ceiling| {
                bar.as_ref()
                    .is_none_or(|current| current.timestamp.date_naive() < ceiling)
            });
            if is_stale && let Some(source) = source {
                match self.ensure_instrument_exists(instrument_id).await {
                    Ok(()) => {
                        if let Err(error) = self.backfill_daily_bars(source, instrument_id).await {
                            tracing::error!(
                                instrument_id,
                                error = %error,
                                "日足データの取得に失敗しました",
                            );
                        }
                        bar = match self.repository.find_latest_bar(instrument_id, "1d").await {
                            Ok(bar) => bar,
                            Err(error) => {
                                tracing::warn!(
                                    instrument_id,
                                    error = %error,
                                    "バックフィル後の直近終値取得に失敗",
                                );
                                None
                            }
                        };
                    }
                    Err(error) => {
                        tracing::warn!(
                            instrument_id,
                            error = %error,
                            "instruments 行の作成に失敗",
                        );
                    }
                }
            }

            if let Some(bar) = bar {
                bars.insert(
                    instrument_id.clone(),
                    (bar.timestamp.date_naive(), bar.close),
                );
            }
        }

        let (prices, priced_at) = select_common_priced_at(bars);
        LatestPrices { prices, priced_at }
    }

    /// DB に保存された日足から、共通観測日の終値を返す。
    pub async fn find_latest_prices(
        &self,
        instrument_ids: &[String],
    ) -> Result<LatestPrices, BarsUseCaseError> {
        let mut bars: HashMap<String, (NaiveDate, Decimal)> = HashMap::new();

        for instrument_id in instrument_ids {
            let latest_bar = self
                .repository
                .find_latest_bar(instrument_id, "1d")
                .await
                .map_err(|error| {
                    tracing::error!(instrument_id, error = %error, "直近終値の取得に失敗");
                    error
                })?;

            if let Some(bar) = latest_bar {
                bars.insert(
                    instrument_id.clone(),
                    (bar.timestamp.date_naive(), bar.close),
                );
            }
        }

        let (prices, priced_at) = select_common_priced_at(bars);
        Ok(LatestPrices { prices, priced_at })
    }

    async fn ensure_instrument_exists(&self, instrument_id: &str) -> Result<(), BarsUseCaseError> {
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .ensure_instruments_exist(
                &transaction,
                &std::collections::HashSet::from([instrument_id.to_string()]),
            )
            .await?;
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

fn latest_fetchable_date(
    today: NaiveDate,
    known_range: Option<(NaiveDate, NaiveDate)>,
) -> NaiveDate {
    known_range.map_or_else(
        || {
            tracing::warn!("取得範囲を公開しないため、上限を today として扱い取得を試みます");
            today
        },
        |(_, to)| to,
    )
}

fn select_common_priced_at(
    bars: HashMap<String, (NaiveDate, Decimal)>,
) -> (HashMap<String, Decimal>, Option<NaiveDate>) {
    let priced_at = bars.values().map(|(date, _)| *date).max();
    let prices = priced_at.map_or_else(HashMap::new, |latest| {
        bars.into_iter()
            .filter(|(instrument_id, (date, _))| {
                if *date == latest {
                    return true;
                }
                tracing::warn!(
                    instrument_id,
                    %date,
                    %latest,
                    "観測日が最新銘柄と食い違うため価格から除外",
                );
                false
            })
            .map(|(instrument_id, (_, close))| (instrument_id, close))
            .collect()
    });
    (prices, priced_at)
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use core_domain::business_day::latest_business_day;
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::bars::{
        BarsByInstrumentsQuery, BarsQuery, BarsRepository, BarsRepositoryError, BarsUseCases,
        DailyBarAdjustmentFactor, FakeBarsRepository, LatestPrices, UsStockBarTarget,
    };
    use crate::daily_bar_source::{DailyBarSource, DailyBarSourceError, DateRange};
    use crate::persistence::PersistenceError;
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork, UnitOfWorkTransaction};

    use super::latest_fetchable_date;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default()
    }

    fn make_bar(instrument_id: &str, date: NaiveDate, close: i64) -> Bar {
        let timestamp = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap_or_default());
        Bar {
            instrument_id: instrument_id.into(),
            timeframe: Timeframe::Daily,
            timestamp,
            open: Decimal::new(close, 0),
            high: Decimal::new(close + 10, 0),
            low: Decimal::new(close - 10, 0),
            close: Decimal::new(close, 0),
            volume: 1000,
            adjustment_factor: Decimal::ONE,
        }
    }

    #[rstest]
    #[case::unknown_range_uses_today(None, date(2037, 4, 14), date(2037, 4, 14))]
    #[case::known_range_uses_its_upper_bound(
        Some((date(2037, 1, 1), date(2037, 2, 1))),
        date(2037, 4, 14),
        date(2037, 2, 1)
    )]
    fn latest_fetchable_date_uses_the_source_upper_bound(
        #[case] range: Option<(NaiveDate, NaiveDate)>,
        #[case] today: NaiveDate,
        #[case] expected: NaiveDate,
    ) {
        assert_eq!(latest_fetchable_date(today, range), expected);
    }

    #[derive(Clone)]
    struct FakeDailyBarSource {
        known_range: Option<(NaiveDate, NaiveDate)>,
        bars_by_instrument: Arc<Mutex<HashMap<String, Vec<Bar>>>>,
        calls: Arc<Mutex<Vec<(String, DateRange)>>>,
        failures: Arc<Mutex<HashSet<String>>>,
    }

    impl FakeDailyBarSource {
        fn new(known_range: Option<(NaiveDate, NaiveDate)>) -> Self {
            Self {
                known_range,
                bars_by_instrument: Arc::new(Mutex::new(HashMap::new())),
                calls: Arc::new(Mutex::new(Vec::new())),
                failures: Arc::new(Mutex::new(HashSet::new())),
            }
        }

        async fn set_bars(&self, instrument_id: &str, bars: Vec<Bar>) {
            self.bars_by_instrument
                .lock()
                .await
                .insert(instrument_id.into(), bars);
        }

        async fn fail_for(&self, instrument_id: &str) {
            self.failures.lock().await.insert(instrument_id.into());
        }
    }

    #[async_trait]
    impl DailyBarSource for FakeDailyBarSource {
        async fn fetch_daily_bars(
            &self,
            instrument_id: &str,
            range: &DateRange,
        ) -> Result<Vec<Bar>, DailyBarSourceError> {
            self.calls
                .lock()
                .await
                .push((instrument_id.into(), range.clone()));
            if self.failures.lock().await.contains(instrument_id) {
                return Err(DailyBarSourceError::Failed("fixture failure".into()));
            }
            Ok(self
                .bars_by_instrument
                .lock()
                .await
                .get(instrument_id)
                .cloned()
                .unwrap_or_default())
        }

        fn known_fetchable_range(&self) -> Option<(NaiveDate, NaiveDate)> {
            self.known_range
        }
    }

    struct BarsFixture {
        use_cases: BarsUseCases,
        unit_of_work: Arc<FakeUnitOfWork>,
        repository: Arc<FakeBarsRepository>,
    }

    #[fixture]
    fn bars_fixture() -> BarsFixture {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeBarsRepository::new());
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let use_cases = BarsUseCases::new(shared_unit_of_work, repository.clone());
        BarsFixture {
            use_cases,
            unit_of_work,
            repository,
        }
    }

    struct FailingBarsRepository;

    #[async_trait]
    impl BarsRepository for FailingBarsRepository {
        async fn find_bars(&self, _: BarsQuery) -> Result<Vec<Bar>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_bars_by_instruments(
            &self,
            _: BarsByInstrumentsQuery,
        ) -> Result<Vec<Bar>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_earliest_us_stock_minute_bar_before(
            &self,
            _: DateTime<Utc>,
        ) -> Result<Option<DateTime<Utc>>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_us_stock_minute_bar_instrument_ids(
            &self,
            _: DateTime<Utc>,
            _: DateTime<Utc>,
            _: DateTime<Utc>,
        ) -> Result<Vec<String>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn downsample_us_stock_minute_bar_range(
            &self,
            _: &UnitOfWorkTransaction,
            _: &str,
            _: DateTime<Utc>,
            _: DateTime<Utc>,
            _: DateTime<Utc>,
        ) -> Result<super::super::types::UsStockMinuteBarsDownsampleStats, BarsRepositoryError>
        {
            Err(database_error())
        }

        async fn find_latest_bar(
            &self,
            _: &str,
            _: &str,
        ) -> Result<Option<Bar>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_daily_adjustment_factors_from(
            &self,
            _: &str,
            _: NaiveDate,
        ) -> Result<Vec<DailyBarAdjustmentFactor>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_us_stock_minute_bar_targets(
            &self,
        ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_us_stock_daily_bar_targets(
            &self,
        ) -> Result<Vec<UsStockBarTarget>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_pending_us_stock_sip_bar_refetch_targets(
            &self,
            _: Option<usize>,
        ) -> Result<Vec<crate::bars::UsStockSipBarRefetchTarget>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn save_us_stock_sip_bar_refetch_cursor(
            &self,
            _: &UnitOfWorkTransaction,
            _: &str,
            _: Option<crate::bars::UsStockSipBarRefetchCursor>,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn mark_us_stock_sip_bar_refetch_completed(
            &self,
            _: &str,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn mark_us_stock_sip_bar_refetch_skipped(
            &self,
            _: &str,
            _: &str,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn reserve_us_stock_sip_refetch_rows(
            &self,
            _: &UnitOfWorkTransaction,
            _: u64,
            _: u64,
            _: chrono::DateTime<Utc>,
            _: u64,
        ) -> Result<bool, BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_us_stock_sip_refetch_next_allowed_at(
            &self,
        ) -> Result<Option<chrono::DateTime<Utc>>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn mark_us_stock_hourly_bar_history_completed(
            &self,
            _: &[String],
            _: chrono::DateTime<Utc>,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn mark_us_stock_minute_bar_history_completed(
            &self,
            _: &[String],
            _: chrono::DateTime<Utc>,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn compress_bars_older_than_policy(
            &self,
            _: chrono::DateTime<Utc>,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn find_ingested_dates(
            &self,
            _: NaiveDate,
        ) -> Result<HashSet<NaiveDate>, BarsRepositoryError> {
            Err(database_error())
        }

        async fn ensure_instruments_exist(
            &self,
            _: &UnitOfWorkTransaction,
            _: &HashSet<String>,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn upsert_bars(
            &self,
            _: &UnitOfWorkTransaction,
            _: Vec<Bar>,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }

        async fn mark_ingested(
            &self,
            _: &UnitOfWorkTransaction,
            _: NaiveDate,
        ) -> Result<(), BarsRepositoryError> {
            Err(database_error())
        }
    }

    fn database_error() -> BarsRepositoryError {
        PersistenceError::Database("synthetic database failure".into()).into()
    }

    #[rstest]
    #[tokio::test]
    async fn backfill_uses_the_fallback_range_when_the_source_has_no_known_range(
        bars_fixture: BarsFixture,
    ) {
        let today = Utc::now().date_naive();
        let bar = make_bar("SAMPLE-GAMMA", today - Duration::days(900), 145);
        let source = FakeDailyBarSource::new(None);
        source.set_bars("SAMPLE-GAMMA", vec![bar.clone()]).await;

        let result = bars_fixture
            .use_cases
            .backfill_daily_bars(&source, "SAMPLE-GAMMA")
            .await;

        assert_eq!(
            (
                result.is_ok(),
                source.calls.lock().await.clone(),
                bars_fixture.repository.bars.lock().await.clone(),
                bars_fixture.unit_of_work.committed.lock().await.len(),
            ),
            (
                true,
                vec![(
                    "SAMPLE-GAMMA".to_string(),
                    DateRange {
                        from: today - Duration::days(365 * 20),
                        to: today,
                    },
                )],
                vec![bar],
                1,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn backfill_does_not_open_a_transaction_when_source_returns_no_bars(
        bars_fixture: BarsFixture,
    ) {
        let today = Utc::now().date_naive();
        let source = FakeDailyBarSource::new(None);

        let result = bars_fixture
            .use_cases
            .backfill_daily_bars(&source, "SAMPLE-GAMMA")
            .await;

        assert_eq!(
            (
                result.is_ok(),
                source.calls.lock().await.clone(),
                bars_fixture.repository.bars.lock().await.clone(),
                bars_fixture.unit_of_work.begun.lock().await.clone(),
                bars_fixture.unit_of_work.committed.lock().await.clone(),
                bars_fixture
                    .repository
                    .write_transaction_ids
                    .lock()
                    .await
                    .clone(),
            ),
            (
                true,
                vec![(
                    "SAMPLE-GAMMA".to_string(),
                    DateRange {
                        from: today - Duration::days(365 * 20),
                        to: today,
                    },
                )],
                vec![],
                vec![],
                vec![],
                vec![],
            ),
        );
    }

    #[test]
    fn select_common_priced_at_excludes_symbols_with_mismatched_dates() {
        let earlier_date = date(2037, 4, 13);
        let latest_date = date(2037, 4, 14);

        assert_eq!(
            super::select_common_priced_at(HashMap::from([
                (
                    "SAMPLE-ALPHA".to_string(),
                    (earlier_date, Decimal::new(100, 0)),
                ),
                (
                    "SAMPLE-BETA".to_string(),
                    (latest_date, Decimal::new(200, 0)),
                ),
            ])),
            (
                HashMap::from([("SAMPLE-BETA".to_string(), Decimal::new(200, 0))]),
                Some(latest_date),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_latest_prices_backfills_stale_bars_and_aligns_the_observation_date(
        bars_fixture: BarsFixture,
    ) {
        let today = Utc::now().date_naive();
        let ceiling = latest_business_day(today);
        let known_range = (ceiling - Duration::days(30), ceiling);
        let stale_bar = make_bar("SAMPLE-ALPHA", ceiling - Duration::days(2), 100);
        let fresh_bar = make_bar("SAMPLE-BETA", ceiling, 210);
        let backfilled_bar = make_bar("SAMPLE-ALPHA", ceiling, 130);
        bars_fixture
            .repository
            .seed_bars(vec![stale_bar.clone(), fresh_bar.clone()])
            .await;
        let source = FakeDailyBarSource::new(Some(known_range));
        source
            .set_bars("SAMPLE-ALPHA", vec![backfilled_bar.clone()])
            .await;

        let result = bars_fixture
            .use_cases
            .fetch_latest_prices(
                Some(&source),
                &["SAMPLE-ALPHA".to_string(), "SAMPLE-BETA".to_string()],
            )
            .await;

        assert_eq!(
            (
                result,
                bars_fixture.repository.bars.lock().await.clone(),
                bars_fixture.repository.instruments.lock().await.clone(),
                source.calls.lock().await.clone(),
                bars_fixture.unit_of_work.committed.lock().await.len(),
                bars_fixture
                    .repository
                    .write_transaction_ids
                    .lock()
                    .await
                    .len(),
            ),
            (
                LatestPrices {
                    prices: HashMap::from([
                        ("SAMPLE-ALPHA".to_string(), Decimal::new(130, 0)),
                        ("SAMPLE-BETA".to_string(), Decimal::new(210, 0)),
                    ]),
                    priced_at: Some(ceiling),
                },
                vec![stale_bar, fresh_bar, backfilled_bar],
                HashSet::from(["SAMPLE-ALPHA".to_string()]),
                vec![(
                    "SAMPLE-ALPHA".to_string(),
                    DateRange {
                        from: known_range.0,
                        to: known_range.1,
                    },
                )],
                2,
                2,
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_latest_prices_returns_no_prices_when_bars_are_missing_without_a_source(
        bars_fixture: BarsFixture,
    ) {
        let result = bars_fixture
            .use_cases
            .fetch_latest_prices(None, &["SAMPLE-MISSING".to_string()])
            .await;

        assert_eq!(
            result,
            LatestPrices {
                prices: HashMap::new(),
                priced_at: None,
            },
        );
    }

    #[tokio::test]
    async fn find_latest_prices_propagates_database_errors() {
        let use_cases = BarsUseCases::new(
            Arc::new(FakeUnitOfWork::new()),
            Arc::new(FailingBarsRepository),
        );
        let result = use_cases
            .find_latest_prices(&["SAMPLE-ALPHA".to_string()])
            .await
            .map_err(|error| error.to_string());

        assert_eq!(result, Err("synthetic database failure".to_string()));
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_latest_prices_keeps_other_symbols_when_backfill_fails(
        bars_fixture: BarsFixture,
    ) {
        let ceiling = latest_business_day(Utc::now().date_naive());
        let known_range = (ceiling - Duration::days(30), ceiling);
        let fresh_bar = make_bar("SAMPLE-BETA", ceiling, 210);
        bars_fixture
            .repository
            .seed_bars(vec![fresh_bar.clone()])
            .await;
        let source = FakeDailyBarSource::new(Some(known_range));
        source.fail_for("SAMPLE-ALPHA").await;

        let result = bars_fixture
            .use_cases
            .fetch_latest_prices(
                Some(&source),
                &["SAMPLE-ALPHA".to_string(), "SAMPLE-BETA".to_string()],
            )
            .await;

        assert_eq!(
            (
                result,
                bars_fixture.repository.bars.lock().await.clone(),
                bars_fixture.repository.instruments.lock().await.clone(),
                source.calls.lock().await.clone(),
                bars_fixture.unit_of_work.committed.lock().await.len(),
                bars_fixture
                    .repository
                    .write_transaction_ids
                    .lock()
                    .await
                    .len(),
            ),
            (
                LatestPrices {
                    prices: HashMap::from([("SAMPLE-BETA".to_string(), Decimal::new(210, 0))]),
                    priced_at: Some(ceiling),
                },
                vec![fresh_bar],
                HashSet::from(["SAMPLE-ALPHA".to_string()]),
                vec![(
                    "SAMPLE-ALPHA".to_string(),
                    DateRange {
                        from: known_range.0,
                        to: known_range.1,
                    },
                )],
                1,
                1,
            ),
        );
    }
}
