use std::collections::HashSet;

use chrono::{Duration, NaiveDate, Utc};
use core_domain::bar::{Bar, Timeframe};
use core_domain::business_day::latest_business_day;

use crate::market_daily_bar_source::MarketDailyBarSource;

use super::error::BarsUseCaseError;
use super::types::IngestStats;
use super::use_cases::BarsUseCases;

const TARGET_BUSINESS_DAYS: usize = 400;
const REFETCH_WINDOW_BUSINESS_DAYS: usize = 7;
const FALLBACK_FETCH_HISTORY_DAYS: i64 = 365 * 20;

impl BarsUseCases {
    /// 日足を取り込む 1 サイクルを実行する。
    pub async fn run_ingest_cycle(
        &self,
        source: &dyn MarketDailyBarSource,
    ) -> Result<IngestStats, BarsUseCaseError> {
        let today = Utc::now().date_naive();
        let Some(range) = source.fetchable_range(today) else {
            tracing::debug!("日足を取得できないため取り込みをスキップします");
            return Ok(IngestStats::default());
        };

        let to = latest_business_day(range.to.min(today));
        let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS);
        let Some(&earliest) = business_days.first() else {
            return Ok(IngestStats::default());
        };

        let ingested = self.repository.find_ingested_dates(earliest).await?;
        let targets = target_dates(&business_days, &ingested);
        let mut stats = IngestStats::default();

        for date in targets {
            stats.days_attempted += 1;
            match self.ingest_date(source, date).await {
                Ok(0) => tracing::debug!(%date, "この日の日足はまだ公開されていません"),
                Ok(count) => stats.bars_upserted += count,
                Err(error) => tracing::warn!(
                    %date,
                    error = %error,
                    "日足の取り込みに失敗、この日をスキップします",
                ),
            }
        }

        Ok(stats)
    }

    async fn ingest_date(
        &self,
        source: &dyn MarketDailyBarSource,
        date: NaiveDate,
    ) -> Result<usize, BarsUseCaseError> {
        let bars = source.fetch_daily_bars_by_date(date).await?;
        if bars.is_empty() {
            return Ok(0);
        }

        let instrument_ids: HashSet<String> =
            bars.iter().map(|bar| bar.instrument_id.clone()).collect();
        let count = bars.len();
        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .ensure_instruments_exist(&transaction, &instrument_ids)
            .await?;
        self.repository.upsert_bars(&transaction, bars).await?;
        self.repository.mark_ingested(&transaction, date).await?;
        self.unit_of_work.commit(transaction).await?;

        Ok(count)
    }

    /// 指定銘柄の日足を取得元の既知範囲、または過去 20 年分から補完する。
    pub async fn backfill_daily_bars(
        &self,
        source: &dyn crate::daily_bar_source::DailyBarSource,
        instrument_id: &str,
    ) -> Result<(), BarsUseCaseError> {
        let today = Utc::now().date_naive();
        let range = source.known_fetchable_range().map_or_else(
            || {
                tracing::warn!("取得範囲を公開しないため、上限を today として扱い取得を試みます");
                crate::daily_bar_source::DateRange {
                    from: today - Duration::days(FALLBACK_FETCH_HISTORY_DAYS),
                    to: today,
                }
            },
            |(from, to)| crate::daily_bar_source::DateRange { from, to },
        );

        let bars = source.fetch_daily_bars(instrument_id, &range).await?;
        if bars.is_empty() {
            tracing::info!(instrument_id, "バックフィル対象のデータがありません");
            return Ok(());
        }

        let daily_bars: Vec<Bar> = bars
            .into_iter()
            .filter(|bar| bar.timeframe == Timeframe::Daily)
            .collect();
        let bar_count = daily_bars.len();
        if bar_count == 0 {
            tracing::info!(
                instrument_id,
                bar_count,
                "日足データのバックフィルが完了しました"
            );
            return Ok(());
        }

        let transaction = self.unit_of_work.begin().await?;
        self.repository
            .upsert_bars(&transaction, daily_bars)
            .await?;
        self.unit_of_work.commit(transaction).await?;

        tracing::info!(
            instrument_id,
            bar_count,
            "日足データのバックフィルが完了しました"
        );
        Ok(())
    }
}

fn recent_business_days(to: NaiveDate, count: usize) -> Vec<NaiveDate> {
    let mut days = Vec::with_capacity(count);
    let mut date = to;
    while days.len() < count {
        if latest_business_day(date) == date {
            days.push(date);
        }
        date -= Duration::days(1);
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

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{NaiveDate, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::rstest;
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::bars::{BarsUseCases, FakeBarsRepository, IngestStats};
    use crate::daily_bar_source::DateRange;
    use crate::market_daily_bar_source::{MarketDailyBarSource, MarketDailyBarSourceError};
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{
        REFETCH_WINDOW_BUSINESS_DAYS, TARGET_BUSINESS_DAYS, recent_business_days, target_dates,
    };

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
        }
    }

    #[rstest]
    #[case::all_unrecorded(
        vec![date(2037, 4, 12), date(2037, 4, 13), date(2037, 4, 14)],
        HashSet::new(),
        vec![date(2037, 4, 12), date(2037, 4, 13), date(2037, 4, 14)]
    )]
    #[case::recorded_before_refetch_window_is_skipped(
        vec![
            date(2037, 4, 1), date(2037, 4, 2), date(2037, 4, 3), date(2037, 4, 4),
            date(2037, 4, 5), date(2037, 4, 6), date(2037, 4, 7), date(2037, 4, 8),
        ],
        HashSet::from([date(2037, 4, 1)]),
        vec![
            date(2037, 4, 2), date(2037, 4, 3), date(2037, 4, 4), date(2037, 4, 5),
            date(2037, 4, 6), date(2037, 4, 7), date(2037, 4, 8),
        ]
    )]
    #[case::recorded_within_refetch_window_is_retried(
        vec![date(2037, 4, 8), date(2037, 4, 9), date(2037, 4, 10)],
        HashSet::from([date(2037, 4, 8), date(2037, 4, 9), date(2037, 4, 10)]),
        vec![date(2037, 4, 8), date(2037, 4, 9), date(2037, 4, 10)]
    )]
    fn target_dates_follow_the_refetch_window(
        #[case] business_days: Vec<NaiveDate>,
        #[case] ingested: HashSet<NaiveDate>,
        #[case] expected: Vec<NaiveDate>,
    ) {
        assert_eq!(target_dates(&business_days, &ingested), expected);
    }

    #[derive(Clone)]
    struct FakeMarketDailyBarSource {
        range: Option<DateRange>,
        bars_by_date: Arc<Mutex<HashMap<NaiveDate, Vec<Bar>>>>,
        failures: Arc<Mutex<HashSet<NaiveDate>>>,
    }

    impl FakeMarketDailyBarSource {
        fn new(to: NaiveDate) -> Self {
            Self {
                range: Some(DateRange {
                    from: to - chrono::Duration::days(500),
                    to,
                }),
                bars_by_date: Arc::new(Mutex::new(HashMap::new())),
                failures: Arc::new(Mutex::new(HashSet::new())),
            }
        }

        async fn set_bars(&self, date: NaiveDate, bars: Vec<Bar>) {
            self.bars_by_date.lock().await.insert(date, bars);
        }

        async fn fail_on(&self, date: NaiveDate) {
            self.failures.lock().await.insert(date);
        }
    }

    #[async_trait]
    impl MarketDailyBarSource for FakeMarketDailyBarSource {
        async fn fetch_daily_bars_by_date(
            &self,
            date: NaiveDate,
        ) -> Result<Vec<Bar>, MarketDailyBarSourceError> {
            if self.failures.lock().await.contains(&date) {
                return Err(MarketDailyBarSourceError::Failed("fixture failure".into()));
            }
            Ok(self
                .bars_by_date
                .lock()
                .await
                .get(&date)
                .cloned()
                .unwrap_or_default())
        }

        fn fetchable_range(&self, _today: NaiveDate) -> Option<DateRange> {
            self.range.clone()
        }
    }

    fn use_cases() -> (BarsUseCases, Arc<FakeUnitOfWork>, Arc<FakeBarsRepository>) {
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let repository = Arc::new(FakeBarsRepository::new());
        let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
        let use_cases = BarsUseCases::new(shared_unit_of_work, repository.clone());
        (use_cases, unit_of_work, repository)
    }

    async fn seed_old_dates(repository: &FakeBarsRepository, to: NaiveDate) -> Vec<NaiveDate> {
        let business_days = recent_business_days(to, TARGET_BUSINESS_DAYS);
        let old_dates = business_days[..business_days.len() - REFETCH_WINDOW_BUSINESS_DAYS]
            .iter()
            .copied()
            .collect();
        repository.seed_ingested_dates(old_dates).await;
        business_days
    }

    #[tokio::test]
    async fn ingest_cycle_saves_bars_and_marks_the_date_in_one_transaction() {
        let to = core_domain::business_day::latest_business_day(Utc::now().date_naive());
        let (use_cases, unit_of_work, repository) = use_cases();
        let source = FakeMarketDailyBarSource::new(to);
        let bar = make_bar("SAMPLE-ALPHA", to, 125);
        source.set_bars(to, vec![bar.clone()]).await;
        seed_old_dates(&repository, to).await;

        let result = use_cases.run_ingest_cycle(&source).await;
        let succeeded = result.is_ok();
        let stats = result.unwrap_or_default();
        let transaction_ids = unit_of_work.begun.lock().await.clone();
        let transaction_id = transaction_ids.first().copied().unwrap_or_default();

        assert_eq!(
            (
                succeeded,
                stats,
                repository.bars.lock().await.clone(),
                repository.ingested_dates.lock().await.contains(&to),
                repository.instruments.lock().await.clone(),
                unit_of_work.committed.lock().await.clone(),
                repository.write_transaction_ids.lock().await.clone(),
            ),
            (
                true,
                IngestStats {
                    days_attempted: REFETCH_WINDOW_BUSINESS_DAYS,
                    bars_upserted: 1,
                },
                vec![bar],
                true,
                HashSet::from(["SAMPLE-ALPHA".to_string()]),
                vec![transaction_id],
                vec![transaction_id; 3],
            ),
        );
    }

    #[tokio::test]
    async fn ingest_cycle_does_not_mark_an_empty_date() {
        let to = core_domain::business_day::latest_business_day(Utc::now().date_naive());
        let (use_cases, unit_of_work, repository) = use_cases();
        let source = FakeMarketDailyBarSource::new(to);
        seed_old_dates(&repository, to).await;

        let result = use_cases.run_ingest_cycle(&source).await;
        let succeeded = result.is_ok();
        let stats = result.unwrap_or_default();

        assert_eq!(
            (
                succeeded,
                stats,
                repository.bars.lock().await.clone(),
                repository.ingested_dates.lock().await.contains(&to),
                unit_of_work.committed.lock().await.len(),
            ),
            (
                true,
                IngestStats {
                    days_attempted: REFETCH_WINDOW_BUSINESS_DAYS,
                    bars_upserted: 0,
                },
                vec![],
                false,
                0,
            ),
        );
    }

    #[tokio::test]
    async fn ingest_cycle_continues_after_a_failed_date() {
        let to = core_domain::business_day::latest_business_day(Utc::now().date_naive());
        let (use_cases, unit_of_work, repository) = use_cases();
        let source = FakeMarketDailyBarSource::new(to);
        let business_days = seed_old_dates(&repository, to).await;
        let previous = business_days[business_days.len() - 2];
        let bar = make_bar("SAMPLE-BETA", previous, 230);
        source.set_bars(previous, vec![bar.clone()]).await;
        source.fail_on(to).await;

        let result = use_cases.run_ingest_cycle(&source).await;
        let succeeded = result.is_ok();
        let stats = result.unwrap_or_default();
        let ingested_dates = repository.ingested_dates.lock().await.clone();

        assert_eq!(
            (
                succeeded,
                stats,
                repository.bars.lock().await.clone(),
                ingested_dates.contains(&to),
                ingested_dates.contains(&previous),
                unit_of_work.committed.lock().await.len(),
            ),
            (
                true,
                IngestStats {
                    days_attempted: REFETCH_WINDOW_BUSINESS_DAYS,
                    bars_upserted: 1,
                },
                vec![bar],
                false,
                true,
                1,
            ),
        );
    }
}
