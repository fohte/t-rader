use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Months, TimeZone, Utc};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;

use super::error::BarsUseCaseError;
use super::types::{BarsByInstrumentsQuery, UsStockBarTarget, UsStockBarsIngestStats};
use super::us_stock_source::{UsStockBarQuery, UsStockBarSource, UsStockBarSourceError};
use super::use_cases::BarsUseCases;

mod split;

const SYMBOLS_PER_REQUEST: usize = 100;
const MINUTE_HISTORY_MONTHS: u32 = 24;
const MAX_REQUESTS_PER_RUN: usize = 5_000;

struct RequestBudget {
    remaining: usize,
}

impl RequestBudget {
    fn new(max_requests: usize) -> Self {
        Self {
            remaining: max_requests,
        }
    }

    fn remaining(&self) -> usize {
        self.remaining
    }

    fn take_one(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }

    fn take(&mut self, requests: usize) -> bool {
        if requests > self.remaining {
            return false;
        }
        self.remaining -= requests;
        true
    }
}

#[derive(Clone, Copy)]
struct UsStockBarRange {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsStockBarTimeframe {
    Daily,
    Minute,
}

struct UsStockSplitHistoryRange {
    daily_from: DateTime<Utc>,
    minute_from: DateTime<Utc>,
    to: DateTime<Utc>,
}

impl UsStockBarTimeframe {
    fn timeframe(self) -> Timeframe {
        match self {
            Self::Daily => Timeframe::Daily,
            Self::Minute => Timeframe::Minute,
        }
    }

    fn latest_bar(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.latest_daily_bar,
            Self::Minute => target.latest_minute_bar,
        }
    }

    fn earliest_bar(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.earliest_daily_bar,
            Self::Minute => target.earliest_minute_bar,
        }
    }
}

impl BarsUseCases {
    pub async fn ingest_us_stock_daily_bars(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestStats, BarsUseCaseError> {
        self.ingest_us_stock_bars_for_timeframe(
            source,
            now,
            UsStockBarTimeframe::Daily,
            MAX_REQUESTS_PER_RUN,
        )
        .await
    }

    pub async fn ingest_us_stock_minute_bars(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestStats, BarsUseCaseError> {
        self.ingest_us_stock_bars_for_timeframe(
            source,
            now,
            UsStockBarTimeframe::Minute,
            MAX_REQUESTS_PER_RUN,
        )
        .await
    }

    async fn ingest_us_stock_bars_for_timeframe(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
        timeframe: UsStockBarTimeframe,
        max_requests: usize,
    ) -> Result<UsStockBarsIngestStats, BarsUseCaseError> {
        let targets = self.repository.find_us_stock_bar_targets().await?;
        let mut stats = UsStockBarsIngestStats {
            symbols_attempted: targets.len(),
            ..UsStockBarsIngestStats::default()
        };
        let history_start = match timeframe {
            UsStockBarTimeframe::Daily => Utc.timestamp_opt(0, 0).single().unwrap_or(now),
            UsStockBarTimeframe::Minute => now
                .checked_sub_months(Months::new(MINUTE_HISTORY_MONTHS))
                .unwrap_or(now),
        };
        let mut budget = RequestBudget::new(max_requests);

        let completed = self
            .ingest_timeframe(
                source,
                &targets,
                timeframe,
                UsStockBarRange {
                    from: history_start,
                    to: now,
                },
                &mut budget,
                &mut stats,
            )
            .await?;

        if completed && timeframe == UsStockBarTimeframe::Daily {
            self.ingest_split_history(
                source,
                &targets,
                history_start.date_naive(),
                now,
                &mut budget,
                &mut stats,
            )
            .await?;
        }

        Ok(stats)
    }

    async fn ingest_timeframe(
        &self,
        source: &dyn UsStockBarSource,
        targets: &[UsStockBarTarget],
        timeframe: UsStockBarTimeframe,
        range: UsStockBarRange,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        let mut missing_history = Vec::new();
        let mut existing_history = Vec::new();
        for target in targets {
            match timeframe.latest_bar(target) {
                Some(timestamp) => existing_history.push((target.instrument_id.clone(), timestamp)),
                None => missing_history.push(target.instrument_id.clone()),
            }
        }

        for instrument_ids in missing_history.chunks(SYMBOLS_PER_REQUEST) {
            if !self
                .ingest_batch(
                    source,
                    instrument_ids,
                    timeframe,
                    UsStockBarRange {
                        from: range.from,
                        to: range.to,
                    },
                    budget,
                    stats,
                )
                .await?
            {
                return Ok(false);
            }
        }

        existing_history.sort_by_key(|(_, timestamp)| std::cmp::Reverse(*timestamp));
        for target_batch in existing_history.chunks(SYMBOLS_PER_REQUEST) {
            let from = target_batch
                .iter()
                .map(|(_, timestamp)| *timestamp)
                .min()
                .unwrap_or(range.from);
            let instrument_ids = target_batch
                .iter()
                .map(|(instrument_id, _)| instrument_id.clone())
                .collect::<Vec<_>>();
            if !self
                .ingest_batch(
                    source,
                    &instrument_ids,
                    timeframe,
                    UsStockBarRange { from, to: range.to },
                    budget,
                    stats,
                )
                .await?
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn ingest_batch(
        &self,
        source: &dyn UsStockBarSource,
        instrument_ids: &[String],
        timeframe: UsStockBarTimeframe,
        range: UsStockBarRange,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        let mut page_token = None;
        loop {
            if !budget.take_one() {
                return Ok(false);
            }
            stats.requests_attempted += 1;
            let page = source
                .fetch_page(&UsStockBarQuery {
                    instrument_ids: instrument_ids.to_vec(),
                    timeframe: timeframe.timeframe(),
                    from: range.from,
                    to: range.to,
                    page_token: page_token.clone(),
                })
                .await?;
            let next_page_token = page.next_page_token;
            let mut bars = page.bars;
            if timeframe == UsStockBarTimeframe::Daily {
                self.preserve_adjustment_factors(&mut bars).await?;
            }
            self.persist_bars(bars, timeframe, stats).await?;

            let Some(next_page_token) = next_page_token else {
                return Ok(true);
            };
            if page_token.as_ref() == Some(&next_page_token) {
                return Err(BarsUseCaseError::UsStockBarSource(
                    UsStockBarSourceError::Failed("pagination token did not advance".to_owned()),
                ));
            }
            page_token = Some(next_page_token);
        }
    }

    async fn fetch_batch_pages(
        &self,
        source: &dyn UsStockBarSource,
        instrument_ids: &[String],
        timeframe: UsStockBarTimeframe,
        range: UsStockBarRange,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<Option<Vec<Vec<Bar>>>, BarsUseCaseError> {
        let mut page_token = None;
        let mut pages = Vec::new();
        loop {
            if !budget.take_one() {
                return Ok(None);
            }
            stats.requests_attempted += 1;
            let page = source
                .fetch_page(&UsStockBarQuery {
                    instrument_ids: instrument_ids.to_vec(),
                    timeframe: timeframe.timeframe(),
                    from: range.from,
                    to: range.to,
                    page_token: page_token.clone(),
                })
                .await?;

            pages.push(page.bars);

            let Some(next_page_token) = page.next_page_token else {
                return Ok(Some(pages));
            };
            if page_token.as_ref() == Some(&next_page_token) {
                return Err(BarsUseCaseError::UsStockBarSource(
                    UsStockBarSourceError::Failed("pagination token did not advance".to_owned()),
                ));
            }
            page_token = Some(next_page_token);
        }
    }

    async fn persist_batch_pages(
        &self,
        pages: Vec<Vec<Bar>>,
        timeframe: UsStockBarTimeframe,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        for page in pages {
            self.persist_bars(page, timeframe, stats).await?;
        }
        Ok(())
    }

    async fn persist_bars(
        &self,
        bars: Vec<Bar>,
        timeframe: UsStockBarTimeframe,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        if bars.is_empty() {
            return Ok(());
        }
        let count = bars.len();
        let transaction = self.unit_of_work.begin().await?;
        match timeframe {
            UsStockBarTimeframe::Daily => {
                self.repository.upsert_bars(&transaction, bars).await?;
                stats.daily_bars_upserted += count;
            }
            UsStockBarTimeframe::Minute => {
                self.repository
                    .upsert_minute_bars(&transaction, bars)
                    .await?;
                stats.minute_bars_upserted += count;
            }
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }

    async fn preserve_adjustment_factors(&self, bars: &mut [Bar]) -> Result<(), BarsUseCaseError> {
        if bars.is_empty() {
            return Ok(());
        }
        let instrument_ids = bars
            .iter()
            .map(|bar| bar.instrument_id.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let from = bars
            .iter()
            .map(|bar| bar.timestamp)
            .min()
            .map(|timestamp| timestamp.fixed_offset());
        let to = bars
            .iter()
            .map(|bar| bar.timestamp)
            .max()
            .map(|timestamp| timestamp.fixed_offset());
        let stored_bars = self
            .repository
            .find_bars_by_instruments(BarsByInstrumentsQuery {
                instrument_ids,
                timeframe: Timeframe::Daily.to_string(),
                from,
                to,
            })
            .await?;
        let stored_factors: HashMap<(String, DateTime<Utc>), Decimal> = stored_bars
            .into_iter()
            .filter(|bar| bar.adjustment_factor != Decimal::ONE)
            .map(|bar| ((bar.instrument_id, bar.timestamp), bar.adjustment_factor))
            .collect();
        for bar in bars {
            if bar.adjustment_factor == Decimal::ONE
                && let Some(factor) =
                    stored_factors.get(&(bar.instrument_id.clone(), bar.timestamp))
            {
                bar.adjustment_factor = *factor;
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::bars::{
        BarsUseCases, FakeBarsRepository, UsStockBarPage, UsStockBarQuery, UsStockBarSource,
        UsStockBarSourceError, UsStockBarsIngestStats, UsStockSplit, UsStockSplitBatch,
        UsStockSplitQuery,
    };
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{MINUTE_HISTORY_MONTHS, UsStockBarTimeframe};

    struct Harness {
        use_cases: BarsUseCases,
        repository: Arc<FakeBarsRepository>,
        source: FakeSource,
    }

    #[fixture]
    fn harness() -> Harness {
        let repository = Arc::new(FakeBarsRepository::new());
        let repository_shared = repository.clone();
        let repository_shared_trait: crate::bars::SharedBarsRepository = repository_shared;
        let unit_of_work = Arc::new(FakeUnitOfWork::new());
        let unit_of_work_shared: SharedUnitOfWork = unit_of_work;
        Harness {
            use_cases: BarsUseCases::new(unit_of_work_shared, repository_shared_trait),
            repository,
            source: FakeSource::default(),
        }
    }

    #[derive(Default)]
    struct FakeSource {
        calls: Mutex<Vec<UsStockBarQuery>>,
        split_calls: Mutex<Vec<UsStockSplitQuery>>,
        splits: Mutex<Vec<UsStockSplit>>,
        bars: Mutex<Vec<Bar>>,
        page_size: Option<usize>,
        repeat_page_token: bool,
        split_has_more: bool,
    }

    #[async_trait]
    impl UsStockBarSource for FakeSource {
        async fn fetch_page(
            &self,
            query: &UsStockBarQuery,
        ) -> Result<UsStockBarPage, UsStockBarSourceError> {
            self.calls.lock().await.push(query.clone());
            let configured_bars = self.bars.lock().await.clone();
            let has_configured_bars = !configured_bars.is_empty();
            let mut matching_bars = if has_configured_bars {
                configured_bars
                    .into_iter()
                    .filter(|bar| {
                        query.instrument_ids.contains(&bar.instrument_id)
                            && bar.timeframe == query.timeframe
                            && bar.timestamp >= query.from
                            && bar.timestamp <= query.to
                    })
                    .collect()
            } else if query.page_token.is_some() {
                Vec::new()
            } else {
                query
                    .instrument_ids
                    .iter()
                    .map(|instrument_id| bar(instrument_id, query.timeframe, query.to))
                    .collect()
            };
            matching_bars.sort_by_key(|bar| bar.timestamp);
            let page_offset = query
                .page_token
                .as_deref()
                .and_then(|token| token.parse::<usize>().ok())
                .unwrap_or_default();
            let page_size = self.page_size.unwrap_or(matching_bars.len().max(1));
            let bars = matching_bars
                .iter()
                .skip(page_offset)
                .take(page_size)
                .cloned()
                .collect();
            let next_page_token = if self.repeat_page_token {
                Some(
                    query
                        .page_token
                        .clone()
                        .unwrap_or_else(|| "synthetic-page".to_owned()),
                )
            } else if has_configured_bars
                && self.page_size.is_some()
                && page_offset.saturating_add(page_size) < matching_bars.len()
            {
                Some(page_offset.saturating_add(page_size).to_string())
            } else if !has_configured_bars {
                (query.timeframe == Timeframe::Daily
                    && query.instrument_ids == vec!["US:QZ7"]
                    && query.page_token.is_none())
                .then(|| "synthetic-page".to_owned())
            } else {
                None
            };
            Ok(UsStockBarPage {
                bars,
                next_page_token,
            })
        }

        async fn fetch_splits(
            &self,
            query: &UsStockSplitQuery,
            max_requests: usize,
        ) -> Result<UsStockSplitBatch, UsStockBarSourceError> {
            self.split_calls.lock().await.push(query.clone());
            Ok(UsStockSplitBatch {
                splits: self
                    .splits
                    .lock()
                    .await
                    .iter()
                    .filter(|split| {
                        query.instrument_ids.contains(&split.instrument_id)
                            && split.ex_date >= query.from
                            && split.ex_date <= query.to
                    })
                    .cloned()
                    .collect(),
                requests_attempted: usize::from(max_requests > 0),
                has_more: self.split_has_more || max_requests == 0,
            })
        }
    }

    fn date(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .map(|timestamp| timestamp.to_utc())
            .unwrap_or_default()
    }

    fn bar(instrument_id: &str, timeframe: Timeframe, timestamp: DateTime<Utc>) -> Bar {
        Bar {
            instrument_id: instrument_id.to_owned(),
            timeframe,
            timestamp,
            open: Decimal::new(101, 1),
            high: Decimal::new(111, 1),
            low: Decimal::new(91, 1),
            close: Decimal::new(105, 1),
            volume: 25,
            adjustment_factor: Decimal::ONE,
        }
    }

    fn priced_bar(
        instrument_id: &str,
        timeframe: Timeframe,
        timestamp: DateTime<Utc>,
        price: i64,
    ) -> Bar {
        let mut bar = bar(instrument_id, timeframe, timestamp);
        bar.open = Decimal::new(price, 0);
        bar.high = Decimal::new(price, 0);
        bar.low = Decimal::new(price, 0);
        bar.close = Decimal::new(price, 0);
        bar
    }

    #[rstest]
    #[tokio::test]
    async fn ingest_backfills_missing_symbols_and_continues_from_saved_bars(harness: Harness) {
        harness
            .repository
            .seed_us_stock_targets(vec!["US:QZ7".to_owned(), "US:QZ-7".to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![
                bar("US:QZ-7", Timeframe::Daily, date("2040-01-01T00:00:00Z")),
                bar("US:QZ-7", Timeframe::Minute, date("2040-01-02T11:59:00Z")),
            ])
            .await;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();
        let daily_stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("US stock daily bar ingestion succeeds");
        let minute_stats = harness
            .use_cases
            .ingest_us_stock_minute_bars(&harness.source, now)
            .await
            .expect("US stock minute bar ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let daily_bars = harness.repository.bars.lock().await.clone();
        let minute_bars = harness.repository.minute_bars.lock().await.clone();
        let expected_minute_start = now
            .checked_sub_months(chrono::Months::new(MINUTE_HISTORY_MONTHS))
            .unwrap_or(now);
        let expected = (
            UsStockBarsIngestStats {
                symbols_attempted: 2,
                requests_attempted: 4,
                daily_bars_upserted: 2,
                minute_bars_upserted: 0,
            },
            UsStockBarsIngestStats {
                symbols_attempted: 2,
                requests_attempted: 2,
                daily_bars_upserted: 0,
                minute_bars_upserted: 2,
            },
            vec![
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: now,
                    page_token: None,
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: now,
                    page_token: Some("synthetic-page".to_owned()),
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("2040-01-01T00:00:00Z"),
                    to: now,
                    page_token: None,
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ7".to_owned()],
                    timeframe: Timeframe::Minute,
                    from: expected_minute_start,
                    to: now,
                    page_token: None,
                },
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    timeframe: Timeframe::Minute,
                    from: date("2040-01-02T11:59:00Z"),
                    to: now,
                    page_token: None,
                },
            ],
            vec![
                bar("US:QZ-7", Timeframe::Daily, date("2040-01-01T00:00:00Z")),
                bar("US:QZ7", Timeframe::Daily, now),
                bar("US:QZ-7", Timeframe::Daily, now),
            ],
            vec![
                bar("US:QZ-7", Timeframe::Minute, date("2040-01-02T11:59:00Z")),
                bar("US:QZ7", Timeframe::Minute, now),
                bar("US:QZ-7", Timeframe::Minute, now),
            ],
        );

        assert_eq!(
            (daily_stats, minute_stats, calls, daily_bars, minute_bars),
            expected,
        );
    }

    #[rstest]
    #[tokio::test]
    async fn groups_existing_symbols_by_their_own_latest_bar(harness: Harness) {
        let instrument_ids = (0..101)
            .map(|index| format!("US:QZ{index:03}"))
            .collect::<Vec<_>>();
        let recent_bar = date("2040-01-01T00:00:00Z");
        let older_bar = date("2039-01-01T00:00:00Z");
        let saved_bars = instrument_ids
            .iter()
            .enumerate()
            .flat_map(|(index, instrument_id)| {
                let timestamp = if index == 0 { older_bar } else { recent_bar };
                [
                    bar(instrument_id, Timeframe::Daily, timestamp),
                    bar(instrument_id, Timeframe::Minute, timestamp),
                ]
            })
            .collect::<Vec<_>>();
        harness
            .repository
            .seed_us_stock_targets(instrument_ids.clone())
            .await;
        harness.repository.seed_bars(saved_bars).await;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();
        harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("US stock daily bar ingestion succeeds");
        harness
            .use_cases
            .ingest_us_stock_minute_bars(&harness.source, now)
            .await
            .expect("US stock minute bar ingestion succeeds");

        let actual = harness.source.calls.lock().await.clone();
        let expected = vec![
            UsStockBarQuery {
                instrument_ids: instrument_ids[1..].to_vec(),
                timeframe: Timeframe::Daily,
                from: recent_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[0].clone()],
                timeframe: Timeframe::Daily,
                from: older_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: instrument_ids[1..].to_vec(),
                timeframe: Timeframe::Minute,
                from: recent_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[0].clone()],
                timeframe: Timeframe::Minute,
                from: older_bar,
                to: now,
                page_token: None,
            },
        ];

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[case::daily(UsStockBarTimeframe::Daily)]
    #[case::minute(UsStockBarTimeframe::Minute)]
    #[tokio::test]
    async fn resumes_from_the_latest_saved_bar_after_reaching_the_request_limit(
        mut harness: Harness,
        #[case] timeframe: UsStockBarTimeframe,
    ) {
        let instrument_id = "US:ZX-42";
        let first_timestamp = date("2040-01-01T00:00:00Z");
        let latest_timestamp = date("2040-01-02T00:00:00Z");
        let final_timestamp = date("2040-01-03T00:00:00Z");
        let now = date("2040-01-04T12:00:00Z");
        let history_start = match timeframe {
            UsStockBarTimeframe::Daily => date("1970-01-01T00:00:00Z"),
            UsStockBarTimeframe::Minute => now
                .checked_sub_months(chrono::Months::new(MINUTE_HISTORY_MONTHS))
                .unwrap_or(now),
        };
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness.source.page_size = Some(2);
        harness.source.bars.lock().await.extend([
            bar(instrument_id, timeframe.timeframe(), first_timestamp),
            bar(instrument_id, timeframe.timeframe(), latest_timestamp),
            bar(instrument_id, timeframe.timeframe(), final_timestamp),
        ]);

        let first_stats = harness
            .use_cases
            .ingest_us_stock_bars_for_timeframe(&harness.source, now, timeframe, 1)
            .await
            .expect("first bounded ingestion succeeds");
        let second_stats = harness
            .use_cases
            .ingest_us_stock_bars_for_timeframe(&harness.source, now, timeframe, 1)
            .await
            .expect("resumed bounded ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let split_calls = harness.source.split_calls.lock().await.clone();
        let mut stored_bars = match timeframe {
            UsStockBarTimeframe::Daily => harness.repository.bars.lock().await.clone(),
            UsStockBarTimeframe::Minute => harness.repository.minute_bars.lock().await.clone(),
        };
        stored_bars.sort_by_key(|bar| bar.timestamp);
        let expected_split_calls = vec![];

        assert_eq!(
            (first_stats, second_stats, calls, split_calls, stored_bars,),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: usize::from(timeframe == UsStockBarTimeframe::Daily) * 2,
                    minute_bars_upserted: usize::from(timeframe == UsStockBarTimeframe::Minute) * 2,
                },
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    daily_bars_upserted: usize::from(timeframe == UsStockBarTimeframe::Daily) * 2,
                    minute_bars_upserted: usize::from(timeframe == UsStockBarTimeframe::Minute) * 2,
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: timeframe.timeframe(),
                        from: history_start,
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: timeframe.timeframe(),
                        from: latest_timestamp,
                        to: now,
                        page_token: None,
                    },
                ],
                expected_split_calls,
                vec![
                    bar(instrument_id, timeframe.timeframe(), first_timestamp),
                    bar(instrument_id, timeframe.timeframe(), latest_timestamp),
                    bar(instrument_id, timeframe.timeframe(), final_timestamp),
                ],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn succeeds_when_the_split_request_limit_is_reached(mut harness: Harness) {
        let instrument_id = "US:RX-45";
        let now = date("2040-01-04T12:00:00Z");
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness.source.split_has_more = true;

        let stats = harness
            .use_cases
            .ingest_us_stock_bars_for_timeframe(&harness.source, now, UsStockBarTimeframe::Daily, 2)
            .await
            .expect("bounded split ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let split_calls = harness.source.split_calls.lock().await.clone();
        let daily_bars = harness.repository.bars.lock().await.clone();

        assert_eq!(
            (stats, calls, split_calls, daily_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 2,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                },
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("1970-01-01T00:00:00Z"),
                    to: now,
                    page_token: None,
                }],
                vec![UsStockSplitQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: now.date_naive(),
                }],
                vec![bar(instrument_id, Timeframe::Daily, now)],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn split_refetch_replaces_full_saved_history_and_sets_ex_date_factor(harness: Harness) {
        let instrument_id = "US:QZ7";
        let earliest_daily = date("2039-12-31T00:00:00Z");
        let ex_date = date("2040-01-01T00:00:00Z");
        let latest_daily = date("2040-01-02T00:00:00Z");
        let earliest_minute = date("2039-12-31T14:30:00Z");
        let latest_minute = date("2040-01-02T20:59:00Z");
        let now = date("2040-01-03T12:00:00Z");
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![
                priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 200),
                priced_bar(instrument_id, Timeframe::Daily, ex_date, 200),
                priced_bar(instrument_id, Timeframe::Daily, latest_daily, 200),
                priced_bar(instrument_id, Timeframe::Minute, earliest_minute, 200),
                priced_bar(instrument_id, Timeframe::Minute, latest_minute, 200),
            ])
            .await;
        let mut adjusted_ex_date = priced_bar(instrument_id, Timeframe::Daily, ex_date, 100);
        adjusted_ex_date.adjustment_factor = Decimal::new(5, 1);
        let adjusted_bars = vec![
            priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 100),
            priced_bar(instrument_id, Timeframe::Daily, ex_date, 100),
            priced_bar(instrument_id, Timeframe::Daily, latest_daily, 100),
            priced_bar(instrument_id, Timeframe::Minute, earliest_minute, 100),
            priced_bar(instrument_id, Timeframe::Minute, latest_minute, 100),
        ];
        harness.source.bars.lock().await.extend(adjusted_bars);
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: NaiveDate::from_ymd_opt(2040, 1, 1).expect("split date is valid"),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("US stock split refetch succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let split_calls = harness.source.split_calls.lock().await.clone();
        let mut daily_bars = harness.repository.bars.lock().await.clone();
        let mut minute_bars = harness.repository.minute_bars.lock().await.clone();
        daily_bars.sort_by_key(|bar| bar.timestamp);
        minute_bars.sort_by_key(|bar| bar.timestamp);

        assert_eq!(
            (stats, calls, split_calls, daily_bars, minute_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 4,
                    daily_bars_upserted: 4,
                    minute_bars_upserted: 2,
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: latest_daily,
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: earliest_daily,
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Minute,
                        from: earliest_minute,
                        to: now,
                        page_token: None,
                    },
                ],
                vec![UsStockSplitQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: NaiveDate::from_ymd_opt(2040, 1, 3).expect("end date is valid"),
                }],
                vec![
                    priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 100),
                    adjusted_ex_date,
                    priced_bar(instrument_id, Timeframe::Daily, latest_daily, 100),
                ],
                vec![
                    priced_bar(instrument_id, Timeframe::Minute, earliest_minute, 100),
                    priced_bar(instrument_id, Timeframe::Minute, latest_minute, 100),
                ],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn defers_split_without_a_saved_ex_date_bar(harness: Harness) {
        let instrument_id = "US:QZ-7";
        let latest_daily = date("2040-01-01T00:00:00Z");
        let latest_minute = date("2040-01-01T14:30:00Z");
        let now = date("2040-01-03T12:00:00Z");
        let saved_bars = vec![
            bar(instrument_id, Timeframe::Daily, latest_daily),
            bar(instrument_id, Timeframe::Minute, latest_minute),
        ];
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness.repository.seed_bars(saved_bars.clone()).await;
        harness.source.bars.lock().await.extend(saved_bars.clone());
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: NaiveDate::from_ymd_opt(2040, 1, 2).expect("split date is valid"),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("US stock bars ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let split_calls = harness.source.split_calls.lock().await.clone();
        let daily_bars = harness.repository.bars.lock().await.clone();
        let minute_bars = harness.repository.minute_bars.lock().await.clone();

        assert_eq!(
            (stats, calls, split_calls, daily_bars, minute_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 2,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                },
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Daily,
                    from: latest_daily,
                    to: now,
                    page_token: None,
                },],
                vec![UsStockSplitQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: NaiveDate::from_ymd_opt(2040, 1, 3).expect("end date is valid"),
                }],
                vec![bar(instrument_id, Timeframe::Daily, latest_daily)],
                vec![bar(instrument_id, Timeframe::Minute, latest_minute)],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn refetches_history_when_split_ex_date_has_no_daily_bar(harness: Harness) {
        let instrument_id = "US:QZ-7";
        let earliest_daily = date("2040-01-01T00:00:00Z");
        let latest_daily = date("2040-01-03T00:00:00Z");
        let earliest_minute = date("2040-01-01T14:30:00Z");
        let latest_minute = date("2040-01-03T20:59:00Z");
        let now = date("2040-01-04T12:00:00Z");
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![
                priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 200),
                priced_bar(instrument_id, Timeframe::Daily, latest_daily, 200),
                priced_bar(instrument_id, Timeframe::Minute, earliest_minute, 200),
                priced_bar(instrument_id, Timeframe::Minute, latest_minute, 200),
            ])
            .await;
        harness.source.bars.lock().await.extend([
            priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 100),
            priced_bar(instrument_id, Timeframe::Daily, latest_daily, 100),
            priced_bar(instrument_id, Timeframe::Minute, earliest_minute, 100),
            priced_bar(instrument_id, Timeframe::Minute, latest_minute, 100),
        ]);
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: NaiveDate::from_ymd_opt(2040, 1, 2).expect("split date is valid"),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("US stock split refetch succeeds without an ex-date bar");
        let calls = harness.source.calls.lock().await.clone();
        let split_calls = harness.source.split_calls.lock().await.clone();
        let mut daily_bars = harness.repository.bars.lock().await.clone();
        let mut minute_bars = harness.repository.minute_bars.lock().await.clone();
        daily_bars.sort_by_key(|bar| bar.timestamp);
        minute_bars.sort_by_key(|bar| bar.timestamp);

        assert_eq!(
            (stats, calls, split_calls, daily_bars, minute_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 4,
                    daily_bars_upserted: 3,
                    minute_bars_upserted: 2,
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: latest_daily,
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: earliest_daily,
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Minute,
                        from: earliest_minute,
                        to: now,
                        page_token: None,
                    },
                ],
                vec![UsStockSplitQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: NaiveDate::from_ymd_opt(2040, 1, 4).expect("end date is valid"),
                }],
                vec![
                    priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 100),
                    priced_bar(instrument_id, Timeframe::Daily, latest_daily, 100),
                ],
                vec![
                    priced_bar(instrument_id, Timeframe::Minute, earliest_minute, 100),
                    priced_bar(instrument_id, Timeframe::Minute, latest_minute, 100),
                ],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn incremental_ingestion_preserves_applied_split_factor(harness: Harness) {
        let instrument_id = "US:QZ-7";
        let now = date("2040-01-03T12:00:00Z");
        let mut split_bar = bar(instrument_id, Timeframe::Daily, now);
        split_bar.adjustment_factor = Decimal::new(5, 1);
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness.repository.seed_bars(vec![split_bar.clone()]).await;
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: NaiveDate::from_ymd_opt(2040, 1, 3).expect("split date is valid"),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("US stock bars ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();
        let split_calls = harness.source.split_calls.lock().await.clone();
        let daily_bars = harness.repository.bars.lock().await.clone();
        let minute_bars = harness.repository.minute_bars.lock().await.clone();
        assert_eq!(
            (stats, calls, split_calls, daily_bars, minute_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 2,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                },
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Daily,
                    from: now,
                    to: now,
                    page_token: None,
                },],
                vec![UsStockSplitQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    from: NaiveDate::from_ymd_opt(1970, 1, 1).expect("start date is valid"),
                    to: NaiveDate::from_ymd_opt(2040, 1, 3).expect("end date is valid"),
                }],
                vec![split_bar],
                vec![],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn fails_when_the_page_token_does_not_advance(harness: Harness) {
        harness
            .repository
            .seed_us_stock_targets(vec!["US:QZ7".to_owned()])
            .await;
        let mut harness = harness;
        harness.source.repeat_page_token = true;
        let now = Utc
            .with_ymd_and_hms(2040, 1, 2, 12, 0, 0)
            .single()
            .unwrap_or_default();

        let result = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .map_err(|error| error.to_string());
        let calls = harness.source.calls.lock().await.clone();

        assert_eq!(
            (result, calls),
            (
                Err("US stock bar source error: pagination token did not advance".to_owned()),
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec!["US:QZ7".to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec!["US:QZ7".to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: Some("synthetic-page".to_owned()),
                    },
                ],
            ),
        );
    }
}
