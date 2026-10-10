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
        let known_range = source.and_then(|source| source.known_fetchable_range());
        let fetchable_ceiling = latest_business_day(latest_fetchable_date(today, known_range));
        let mut bars: HashMap<String, (NaiveDate, Decimal)> = HashMap::new();

        for instrument_id in instrument_ids {
            let mut bar = match self.repository.find_latest_bar(instrument_id, "1d").await {
                Ok(bar) => bar,
                Err(error) => {
                    tracing::warn!(instrument_id, error = %error, "直近終値の取得に失敗");
                    None
                }
            };

            let is_stale = bar
                .as_ref()
                .is_none_or(|current| current.timestamp.date_naive() < fetchable_ceiling);
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
    use chrono::{Duration, NaiveDate, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use core_domain::business_day::latest_business_day;
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::bars::{BarsUseCases, FakeBarsRepository, LatestPrices};
    use crate::daily_bar_source::{DailyBarSource, DailyBarSourceError, DateRange};
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

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
