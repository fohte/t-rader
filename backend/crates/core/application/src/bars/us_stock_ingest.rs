use std::collections::HashSet;

use chrono::{DateTime, Months, TimeZone, Utc};
use core_domain::bar::{Bar, Timeframe};

use super::error::BarsUseCaseError;
use super::types::{UsStockBarTarget, UsStockBarsIngestStats};
use super::us_stock_hourly_history_cutoff;
use super::us_stock_source::{
    SkippedUsStockInstrument, UsStockBarQuery, UsStockBarSource, UsStockBarSourceError,
};
use super::use_cases::BarsUseCases;

mod adjustment;
mod hourly;
mod split;

const SYMBOLS_PER_REQUEST: usize = 100;
const INTRADAY_HISTORY_BOUNDARY_MONTHS: u32 = 12;
const MAX_REQUESTS_PER_RUN: usize = 5_000;

struct RequestBudget {
    remaining: usize,
}

struct BatchIngestContext<'a> {
    budget: &'a mut RequestBudget,
    stats: &'a mut UsStockBarsIngestStats,
    skipped_instrument_ids: HashSet<String>,
}

impl<'a> BatchIngestContext<'a> {
    fn new(budget: &'a mut RequestBudget, stats: &'a mut UsStockBarsIngestStats) -> Self {
        Self {
            budget,
            stats,
            skipped_instrument_ids: HashSet::new(),
        }
    }

    fn record_skipped(&mut self, instrument_id: String, reason: String) {
        self.skipped_instrument_ids.insert(instrument_id.clone());
        record_skipped_instrument(self.stats, instrument_id, reason);
    }
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
    Hourly,
}

struct UsStockSplitHistoryRange {
    daily_from: DateTime<Utc>,
    minute_from: DateTime<Utc>,
    hourly_from: DateTime<Utc>,
    to: DateTime<Utc>,
    hourly_to: DateTime<Utc>,
}

impl UsStockBarTimeframe {
    fn timeframe(self) -> Timeframe {
        match self {
            Self::Daily => Timeframe::Daily,
            Self::Minute => Timeframe::Minute,
            Self::Hourly => Timeframe::Hourly,
        }
    }

    fn history_cursor(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.latest_daily_bar,
            Self::Minute => target.latest_minute_bar,
            Self::Hourly => target.latest_hourly_bar,
        }
    }

    fn earliest_bar(self, target: &UsStockBarTarget) -> Option<DateTime<Utc>> {
        match self {
            Self::Daily => target.earliest_daily_bar,
            Self::Minute => target.earliest_minute_bar,
            Self::Hourly => target.earliest_hourly_bar,
        }
    }

    fn history_start(self, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Daily => Utc.timestamp_opt(0, 0).single().unwrap_or(now),
            Self::Minute => now
                .checked_sub_months(Months::new(INTRADAY_HISTORY_BOUNDARY_MONTHS))
                .unwrap_or(now),
            Self::Hourly => Utc
                .with_ymd_and_hms(2016, 1, 1, 0, 0, 0)
                .single()
                .unwrap_or(now),
        }
    }

    fn history_end(self, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Hourly => us_stock_hourly_history_cutoff(now),
            Self::Daily | Self::Minute => now,
        }
    }

    fn is_complete(self, target: &UsStockBarTarget) -> bool {
        // API は初回の履歴取得に使い、その後の時間足は古くなった分足から補う。
        self == Self::Hourly && target.has_hourly_history_backfill()
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
        let mut ingest_result = self
            .ingest_us_stock_bars_for_timeframe(
                source,
                now,
                UsStockBarTimeframe::Minute,
                MAX_REQUESTS_PER_RUN,
            )
            .await;
        if let Err(error) = self.repository.compress_bars_older_than_policy(now).await {
            let compression_error = error.to_string();
            if let Ok(stats) = &mut ingest_result {
                stats.compression_error = Some(compression_error);
            } else {
                tracing::error!(error = %compression_error, "米国株の分足取り込み後に古い bars chunk を圧縮できませんでした");
            }
        }
        ingest_result
    }

    pub async fn ingest_us_stock_hourly_bars(
        &self,
        source: &dyn UsStockBarSource,
        now: DateTime<Utc>,
    ) -> Result<UsStockBarsIngestStats, BarsUseCaseError> {
        self.ingest_us_stock_bars_for_timeframe(
            source,
            now,
            UsStockBarTimeframe::Hourly,
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
        let targets = match timeframe {
            UsStockBarTimeframe::Daily | UsStockBarTimeframe::Hourly => {
                self.repository.find_us_stock_daily_bar_targets().await?
            }
            UsStockBarTimeframe::Minute => {
                self.repository.find_us_stock_minute_bar_targets().await?
            }
        };
        let mut stats = UsStockBarsIngestStats {
            symbols_attempted: targets.len(),
            ..UsStockBarsIngestStats::default()
        };
        let mut budget = RequestBudget::new(max_requests);

        let completed = self
            .ingest_timeframe(
                source,
                &targets,
                timeframe,
                UsStockBarRange {
                    from: timeframe.history_start(now),
                    to: timeframe.history_end(now),
                },
                &mut budget,
                &mut stats,
            )
            .await?;

        let completed = if completed && timeframe == UsStockBarTimeframe::Daily {
            self.ingest_split_history(
                source,
                &targets,
                UsStockSplitHistoryRange {
                    daily_from: timeframe.history_start(now),
                    minute_from: UsStockBarTimeframe::Minute.history_start(now),
                    hourly_from: UsStockBarTimeframe::Hourly.history_start(now),
                    to: now,
                    hourly_to: UsStockBarTimeframe::Hourly.history_end(now),
                },
                &mut budget,
                &mut stats,
            )
            .await?
        } else {
            completed
        };
        if !completed {
            stats.interrupted_reason = Some("request_budget_exhausted".to_owned());
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
            if timeframe.is_complete(target) {
                continue;
            }
            match timeframe.history_cursor(target) {
                Some(timestamp) => existing_history.push((target.instrument_id.clone(), timestamp)),
                None => missing_history.push(target.instrument_id.clone()),
            }
        }

        existing_history.sort_by_key(|(_, timestamp)| *timestamp);
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
                .ingest_batch_and_mark_progress(
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

        for instrument_ids in missing_history.chunks(SYMBOLS_PER_REQUEST) {
            if !self
                .ingest_batch_and_mark_progress(
                    source,
                    instrument_ids,
                    timeframe,
                    range,
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

    async fn ingest_batch_and_mark_progress(
        &self,
        source: &dyn UsStockBarSource,
        instrument_ids: &[String],
        timeframe: UsStockBarTimeframe,
        range: UsStockBarRange,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        let (completed, skipped_instrument_ids) = {
            let mut context = BatchIngestContext::new(budget, stats);
            let completed = self
                .ingest_batch(source, instrument_ids, timeframe, range, &mut context)
                .await?;
            (completed, context.skipped_instrument_ids)
        };
        if completed && timeframe == UsStockBarTimeframe::Hourly {
            self.mark_hourly_history_completed(instrument_ids, range.to, &skipped_instrument_ids)
                .await?;
        }
        Ok(completed)
    }

    async fn ingest_batch(
        &self,
        source: &dyn UsStockBarSource,
        instrument_ids: &[String],
        timeframe: UsStockBarTimeframe,
        range: UsStockBarRange,
        context: &mut BatchIngestContext<'_>,
    ) -> Result<bool, BarsUseCaseError> {
        let mut page_token = None;
        loop {
            if !context.budget.take(1) {
                return Ok(false);
            }
            context.stats.requests_attempted += 1;
            let page = match source
                .fetch_page(&UsStockBarQuery {
                    instrument_ids: instrument_ids.to_vec(),
                    timeframe: timeframe.timeframe(),
                    from: range.from,
                    to: range.to,
                    page_token: page_token.clone(),
                })
                .await
            {
                Ok(page) => page,
                Err(UsStockBarSourceError::RejectedSymbols(reason)) => {
                    let (first, second) = match split_rejected_batch(instrument_ids, reason) {
                        RejectedBatch::Split(first, second) => (first, second),
                        RejectedBatch::Skipped {
                            instrument_id,
                            reason,
                        } => {
                            context.record_skipped(instrument_id, reason);
                            return Ok(true);
                        }
                    };
                    if !Box::pin(self.ingest_batch(source, first, timeframe, range, context))
                        .await?
                    {
                        return Ok(false);
                    }
                    return Box::pin(self.ingest_batch(source, second, timeframe, range, context))
                        .await;
                }
                Err(error) => return Err(error.into()),
            };
            for skipped in page.skipped_instruments {
                context.record_skipped(skipped.instrument_id, skipped.reason);
            }
            let next_page_token = page.next_page_token;
            let mut bars = page.bars;
            if timeframe == UsStockBarTimeframe::Daily {
                self.preserve_adjustment_factors(&mut bars).await?;
            }
            self.persist_bars(bars, timeframe, context.stats).await?;

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
        self.repository.upsert_bars(&transaction, bars).await?;
        match timeframe {
            UsStockBarTimeframe::Daily => stats.daily_bars_upserted += count,
            UsStockBarTimeframe::Minute => stats.minute_bars_upserted += count,
            UsStockBarTimeframe::Hourly => stats.hourly_bars_upserted += count,
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(())
    }
}

pub(super) fn record_skipped_instrument(
    stats: &mut UsStockBarsIngestStats,
    instrument_id: String,
    reason: String,
) {
    if stats
        .skipped_instruments
        .iter()
        .any(|skipped| skipped.instrument_id == instrument_id)
    {
        return;
    }
    tracing::warn!(
        instrument_id,
        reason,
        "米国株の足取り込みで対応できない銘柄をスキップしました"
    );
    stats.skipped_instruments.push(SkippedUsStockInstrument {
        instrument_id,
        reason,
    });
}

enum RejectedBatch<'a> {
    Split(&'a [String], &'a [String]),
    Skipped {
        instrument_id: String,
        reason: String,
    },
}

fn split_rejected_batch<'a>(instrument_ids: &'a [String], reason: String) -> RejectedBatch<'a> {
    if instrument_ids.len() == 1 {
        return RejectedBatch::Skipped {
            instrument_id: instrument_ids[0].clone(),
            reason,
        };
    }
    let split_at = instrument_ids.len() / 2;
    let (first, second) = instrument_ids.split_at(split_at);
    RejectedBatch::Split(first, second)
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use async_trait::async_trait;
    use chrono::{DateTime, NaiveDate, TimeZone, Utc};
    use core_domain::bar::{Bar, Timeframe};
    use rstest::{fixture, rstest};
    use rust_decimal::Decimal;
    use tokio::sync::Mutex;

    use crate::bars::{
        BarsUseCases, FakeBarsRepository, SkippedUsStockInstrument, UsStockBarPage,
        UsStockBarQuery, UsStockBarSource, UsStockBarSourceError, UsStockBarsIngestStats,
        UsStockSplit, UsStockSplitBatch, UsStockSplitQuery,
    };
    use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};

    use super::{INTRADAY_HISTORY_BOUNDARY_MONTHS, MAX_REQUESTS_PER_RUN};

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
        pagination: Option<PaginatedPages>,
        rejected_instruments: Vec<String>,
        skipped_instruments: Vec<String>,
        rejected_split_instruments: Vec<String>,
        fetch_error: Option<String>,
    }

    struct PaginatedPages {
        instrument_id: String,
        from: DateTime<Utc>,
        total_pages: usize,
        first_page_bars: Vec<Bar>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct BarQueryRun {
        instrument_ids: Vec<String>,
        timeframe: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        page_count: usize,
        first_page_token: Option<String>,
        last_page_token: Option<String>,
        page_tokens_advance: bool,
    }

    #[async_trait]
    impl UsStockBarSource for FakeSource {
        async fn fetch_page(
            &self,
            query: &UsStockBarQuery,
        ) -> Result<UsStockBarPage, UsStockBarSourceError> {
            self.calls.lock().await.push(query.clone());
            if let Some(message) = &self.fetch_error {
                return Err(UsStockBarSourceError::Failed(message.clone()));
            }
            if query
                .instrument_ids
                .iter()
                .any(|instrument_id| self.rejected_instruments.contains(instrument_id))
            {
                return Err(UsStockBarSourceError::RejectedSymbols(
                    "synthetic symbol rejection".to_owned(),
                ));
            }
            let skipped_instruments = query
                .instrument_ids
                .iter()
                .filter(|instrument_id| self.skipped_instruments.contains(*instrument_id))
                .map(|instrument_id| SkippedUsStockInstrument {
                    instrument_id: instrument_id.clone(),
                    reason: "synthetic symbol conversion failure".to_owned(),
                })
                .collect::<Vec<_>>();
            let skipped_instrument_ids = skipped_instruments
                .iter()
                .map(|skipped| skipped.instrument_id.as_str())
                .collect::<HashSet<_>>();
            if let Some(pagination) = &self.pagination
                && query.instrument_ids.len() == 1
                && query.instrument_ids.first() == Some(&pagination.instrument_id)
                && query.from == pagination.from
            {
                let page_index = query
                    .page_token
                    .as_deref()
                    .and_then(|token| token.parse::<usize>().ok())
                    .unwrap_or_default();
                let next_page = page_index.saturating_add(1);
                return Ok(UsStockBarPage {
                    bars: if page_index == 0 {
                        pagination.first_page_bars.clone()
                    } else {
                        Vec::new()
                    },
                    next_page_token: (next_page < pagination.total_pages)
                        .then(|| next_page.to_string()),
                    skipped_instruments,
                });
            }
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
            matching_bars
                .retain(|bar| !skipped_instrument_ids.contains(bar.instrument_id.as_str()));
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
                skipped_instruments,
            })
        }

        async fn fetch_splits(
            &self,
            query: &UsStockSplitQuery,
            max_requests: usize,
        ) -> Result<UsStockSplitBatch, UsStockBarSourceError> {
            self.split_calls.lock().await.push(query.clone());
            if query
                .instrument_ids
                .iter()
                .any(|instrument_id| self.rejected_split_instruments.contains(instrument_id))
            {
                return Err(UsStockBarSourceError::RejectedSymbols(
                    "synthetic split rejection".to_owned(),
                ));
            }
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
                skipped_instruments: Vec::new(),
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

    fn partition_daily_bars(bars: Vec<Bar>) -> (Vec<Bar>, Vec<Bar>) {
        bars.into_iter()
            .partition(|bar| bar.timeframe == Timeframe::Daily)
    }

    fn summarize_query_runs(calls: &[UsStockBarQuery]) -> Vec<BarQueryRun> {
        let mut runs: Vec<BarQueryRun> = Vec::new();
        for call in calls {
            let same_run = runs.last().is_some_and(|run| {
                run.instrument_ids == call.instrument_ids
                    && run.timeframe == call.timeframe
                    && run.from == call.from
                    && run.to == call.to
            });
            if !same_run {
                runs.push(BarQueryRun {
                    instrument_ids: call.instrument_ids.clone(),
                    timeframe: call.timeframe,
                    from: call.from,
                    to: call.to,
                    page_count: 0,
                    first_page_token: call.page_token.clone(),
                    last_page_token: call.page_token.clone(),
                    page_tokens_advance: true,
                });
            }
            if let Some(run) = runs.last_mut() {
                run.page_tokens_advance &= match (run.page_count, call.page_token.as_deref()) {
                    (0, None) => true,
                    (page_index, Some(token)) => token == page_index.to_string(),
                    _ => false,
                };
                run.page_count += 1;
                run.last_page_token = call.page_token.clone();
            }
        }
        runs
    }

    #[rstest]
    #[tokio::test]
    async fn hourly_backfill_uses_the_twelve_month_boundary_and_skips_completed_targets_later(
        harness: Harness,
    ) {
        let instrument_id = "US:DEMO-HOURLY";
        let first_now = date("2040-03-02T12:00:00Z");
        let hourly_end = date("2039-03-02T12:00:00Z");
        harness
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;

        let first_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, first_now)
            .await
            .expect("hourly backfill succeeds");
        let first_calls = harness.source.calls.lock().await.clone();
        harness.source.calls.lock().await.clear();
        let later_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, date("2040-03-02T13:00:00Z"))
            .await
            .expect("completed hourly backfill is skipped");
        let later_calls = harness.source.calls.lock().await.clone();
        let completed_through = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .get(instrument_id)
            .copied();

        assert_eq!(
            (
                first_stats,
                first_calls,
                later_stats,
                later_calls,
                completed_through,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    hourly_bars_upserted: 1,
                    ..UsStockBarsIngestStats::default()
                },
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Hourly,
                    from: date("2016-01-01T00:00:00Z"),
                    to: hourly_end,
                    page_token: None,
                }],
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    ..UsStockBarsIngestStats::default()
                },
                vec![],
                Some(hourly_end),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn hourly_backfill_retries_rejected_and_skipped_instruments(mut harness: Harness) {
        let rejected_id = "US:QZ-REJECTED";
        let skipped_id = "US:QZ-SKIPPED";
        let supported_id = "US:QZ-SUPPORTED";
        let now = date("2040-03-02T12:00:00Z");
        let hourly_end = date("2039-03-02T12:00:00Z");
        harness
            .repository
            .seed_us_stock_daily_targets(vec![
                rejected_id.to_owned(),
                skipped_id.to_owned(),
                supported_id.to_owned(),
            ])
            .await;
        harness.source.rejected_instruments = vec![rejected_id.to_owned()];
        harness.source.skipped_instruments = vec![skipped_id.to_owned()];

        let first_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, now)
            .await
            .expect("rejected and skipped symbols do not fail hourly ingestion");
        let first_calls = harness.source.calls.lock().await.clone();
        let first_progress = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .clone();
        harness.source.calls.lock().await.clear();
        let retry_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, now)
            .await
            .expect("rejected and skipped symbols remain retryable");
        let retry_calls = harness.source.calls.lock().await.clone();
        let retry_progress = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .clone();

        let query = |instrument_ids: Vec<String>| UsStockBarQuery {
            instrument_ids,
            timeframe: Timeframe::Hourly,
            from: date("2016-01-01T00:00:00Z"),
            to: hourly_end,
            page_token: None,
        };
        assert_eq!(
            (
                first_stats,
                first_calls,
                first_progress,
                retry_stats,
                retry_calls,
                retry_progress,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 3,
                    requests_attempted: 3,
                    hourly_bars_upserted: 1,
                    skipped_instruments: vec![
                        SkippedUsStockInstrument {
                            instrument_id: rejected_id.to_owned(),
                            reason: "synthetic symbol rejection".to_owned(),
                        },
                        SkippedUsStockInstrument {
                            instrument_id: skipped_id.to_owned(),
                            reason: "synthetic symbol conversion failure".to_owned(),
                        },
                    ],
                    ..UsStockBarsIngestStats::default()
                },
                vec![
                    query(vec![
                        rejected_id.to_owned(),
                        skipped_id.to_owned(),
                        supported_id.to_owned(),
                    ]),
                    query(vec![rejected_id.to_owned()]),
                    query(vec![skipped_id.to_owned(), supported_id.to_owned()]),
                ],
                HashMap::from([(supported_id.to_owned(), hourly_end)]),
                UsStockBarsIngestStats {
                    symbols_attempted: 3,
                    requests_attempted: 3,
                    skipped_instruments: vec![
                        SkippedUsStockInstrument {
                            instrument_id: rejected_id.to_owned(),
                            reason: "synthetic symbol rejection".to_owned(),
                        },
                        SkippedUsStockInstrument {
                            instrument_id: skipped_id.to_owned(),
                            reason: "synthetic symbol conversion failure".to_owned(),
                        },
                    ],
                    ..UsStockBarsIngestStats::default()
                },
                vec![
                    query(vec![rejected_id.to_owned(), skipped_id.to_owned()]),
                    query(vec![rejected_id.to_owned()]),
                    query(vec![skipped_id.to_owned()]),
                ],
                HashMap::from([(supported_id.to_owned(), hourly_end)]),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn hourly_backfill_resumes_after_reaching_the_request_limit(mut harness: Harness) {
        let instrument_id = "US:DEMO-HOURLY";
        let history_start = date("2016-01-01T00:00:00Z");
        let saved_bar = date("2016-01-02T00:00:00Z");
        let now = date("2040-03-02T12:00:00Z");
        let hourly_end = date("2039-03-02T12:00:00Z");
        harness
            .repository
            .seed_us_stock_daily_targets(vec![instrument_id.to_owned()])
            .await;
        harness.source.pagination = Some(PaginatedPages {
            instrument_id: instrument_id.to_owned(),
            from: history_start,
            total_pages: MAX_REQUESTS_PER_RUN + 1,
            first_page_bars: vec![bar(instrument_id, Timeframe::Hourly, saved_bar)],
        });

        let first_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, now)
            .await
            .expect("request budget exhaustion is reported in stats");
        let second_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, now)
            .await
            .expect("hourly backfill resumes from the saved bar");
        let calls = summarize_query_runs(&harness.source.calls.lock().await);
        let completed_through = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .get(instrument_id)
            .copied();

        assert_eq!(
            (first_stats, second_stats, calls, completed_through,),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: MAX_REQUESTS_PER_RUN,
                    hourly_bars_upserted: 1,
                    interrupted_reason: Some("request_budget_exhausted".to_owned()),
                    ..UsStockBarsIngestStats::default()
                },
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    hourly_bars_upserted: 1,
                    ..UsStockBarsIngestStats::default()
                },
                vec![
                    BarQueryRun {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Hourly,
                        from: history_start,
                        to: hourly_end,
                        page_count: MAX_REQUESTS_PER_RUN,
                        first_page_token: None,
                        last_page_token: Some((MAX_REQUESTS_PER_RUN - 1).to_string()),
                        page_tokens_advance: true,
                    },
                    BarQueryRun {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Hourly,
                        from: saved_bar,
                        to: hourly_end,
                        page_count: 1,
                        first_page_token: None,
                        last_page_token: None,
                        page_tokens_advance: true,
                    },
                ],
                Some(hourly_end),
            ),
        );
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
        let (daily_bars, minute_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());
        let expected_minute_start = now
            .checked_sub_months(chrono::Months::new(INTRADAY_HISTORY_BOUNDARY_MONTHS))
            .unwrap_or(now);
        let expected = (
            UsStockBarsIngestStats {
                symbols_attempted: 2,
                requests_attempted: 4,
                daily_bars_upserted: 2,
                minute_bars_upserted: 0,
                hourly_bars_upserted: 0,
                skipped_instruments: Vec::new(),
                interrupted_reason: None,
                compression_error: None,
            },
            UsStockBarsIngestStats {
                symbols_attempted: 2,
                requests_attempted: 2,
                daily_bars_upserted: 0,
                minute_bars_upserted: 2,
                hourly_bars_upserted: 0,
                skipped_instruments: Vec::new(),
                interrupted_reason: None,
                compression_error: None,
            },
            vec![
                UsStockBarQuery {
                    instrument_ids: vec!["US:QZ-7".to_owned()],
                    timeframe: Timeframe::Daily,
                    from: date("2040-01-01T00:00:00Z"),
                    to: now,
                    page_token: None,
                },
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
                    timeframe: Timeframe::Minute,
                    from: date("2040-01-02T11:59:00Z"),
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
            ],
            vec![
                bar("US:QZ-7", Timeframe::Daily, date("2040-01-01T00:00:00Z")),
                bar("US:QZ-7", Timeframe::Daily, now),
                bar("US:QZ7", Timeframe::Daily, now),
            ],
            vec![
                bar("US:QZ-7", Timeframe::Minute, date("2040-01-02T11:59:00Z")),
                bar("US:QZ-7", Timeframe::Minute, now),
                bar("US:QZ7", Timeframe::Minute, now),
            ],
        );

        assert_eq!(
            (daily_stats, minute_stats, calls, daily_bars, minute_bars),
            expected,
        );
    }

    #[rstest]
    #[tokio::test]
    async fn minute_ingestion_uses_repository_selected_minute_targets(harness: Harness) {
        harness
            .repository
            .seed_us_stock_targets(vec!["US:QZ-MINUTE".to_owned()])
            .await;
        harness
            .repository
            .seed_us_stock_minute_targets(vec!["US:QZ-DAILY".to_owned()])
            .await;
        harness
            .repository
            .seed_us_stock_daily_targets(vec!["US:QZ-DAILY".to_owned()])
            .await;
        let now = date("2040-01-02T12:00:00Z");

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
        let hourly_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, now)
            .await
            .expect("US stock hourly bar ingestion succeeds");
        let calls = harness.source.calls.lock().await.clone();

        assert_eq!(
            (
                daily_stats.symbols_attempted,
                minute_stats.symbols_attempted,
                hourly_stats.symbols_attempted,
                calls,
            ),
            (
                2,
                2,
                2,
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec!["US:QZ-DAILY".to_owned(), "US:QZ-MINUTE".to_owned(),],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec!["US:QZ-DAILY".to_owned(), "US:QZ-MINUTE".to_owned()],
                        timeframe: Timeframe::Minute,
                        from: date("2039-01-02T12:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec!["US:QZ-DAILY".to_owned(), "US:QZ-MINUTE".to_owned(),],
                        timeframe: Timeframe::Hourly,
                        from: date("2016-01-01T00:00:00Z"),
                        to: date("2039-01-02T12:00:00Z"),
                        page_token: None,
                    },
                ],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn minute_ingestion_requests_compression_using_current_time(harness: Harness) {
        let now = date("2040-01-02T12:00:00Z");

        harness
            .use_cases
            .ingest_us_stock_minute_bars(&harness.source, now)
            .await
            .expect("US stock minute bar ingestion succeeds");

        assert_eq!(
            *harness.repository.bar_compression_times.lock().await,
            vec![now],
        );
    }

    #[rstest]
    #[tokio::test]
    async fn minute_ingestion_returns_ingest_stats_when_compression_fails(harness: Harness) {
        let now = date("2040-01-02T12:00:00Z");
        harness
            .repository
            .fail_bar_compression_with("compression failed")
            .await;

        let stats = harness
            .use_cases
            .ingest_us_stock_minute_bars(&harness.source, now)
            .await
            .expect("compression failure does not mask minute ingestion stats");

        assert_eq!(
            stats,
            UsStockBarsIngestStats {
                compression_error: Some("compression failed".to_owned()),
                ..UsStockBarsIngestStats::default()
            },
        );
    }

    #[rstest]
    #[tokio::test]
    async fn minute_ingestion_preserves_ingest_error_when_compression_fails(mut harness: Harness) {
        let now = date("2040-01-02T12:00:00Z");
        harness
            .repository
            .seed_us_stock_targets(vec!["US:QZ-MINUTE".to_owned()])
            .await;
        harness.source.fetch_error = Some("minute fetch failed".to_owned());
        harness
            .repository
            .fail_bar_compression_with("compression failed")
            .await;

        let actual = (
            harness
                .use_cases
                .ingest_us_stock_minute_bars(&harness.source, now)
                .await
                .expect_err("minute source failure is returned"),
            harness
                .repository
                .bar_compression_times
                .lock()
                .await
                .clone(),
        );

        assert_eq!(
            (actual.0.to_string(), actual.1),
            (
                "US stock bar source error: minute fetch failed".to_owned(),
                vec![now]
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn skips_an_alpaca_rejected_symbol_and_ingests_the_rest(mut harness: Harness) {
        let rejected_id = "US:QZ-UNSUPPORTED";
        let supported_id = "US:QZ-VALID";
        harness
            .repository
            .seed_us_stock_targets(vec![rejected_id.to_owned(), supported_id.to_owned()])
            .await;
        harness.source.rejected_instruments = vec![rejected_id.to_owned()];
        let now = date("2040-01-02T12:00:00Z");

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("one rejected symbol does not fail daily ingestion");
        let actual = (
            stats,
            harness.source.calls.lock().await.clone(),
            harness.repository.bars.lock().await.clone(),
        );

        assert_eq!(
            actual,
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 2,
                    requests_attempted: 4,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    skipped_instruments: vec![SkippedUsStockInstrument {
                        instrument_id: rejected_id.to_owned(),
                        reason: "synthetic symbol rejection".to_owned(),
                    }],
                    interrupted_reason: None,
                    compression_error: None,
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![rejected_id.to_owned(), supported_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![rejected_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![supported_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                ],
                vec![bar(supported_id, Timeframe::Daily, now)],
            ),
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
                instrument_ids: instrument_ids[..100].to_vec(),
                timeframe: Timeframe::Daily,
                from: older_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[100].clone()],
                timeframe: Timeframe::Daily,
                from: recent_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: instrument_ids[..100].to_vec(),
                timeframe: Timeframe::Minute,
                from: older_bar,
                to: now,
                page_token: None,
            },
            UsStockBarQuery {
                instrument_ids: vec![instrument_ids[100].clone()],
                timeframe: Timeframe::Minute,
                from: recent_bar,
                to: now,
                page_token: None,
            },
        ];

        assert_eq!(actual, expected);
    }

    #[rstest]
    #[tokio::test]
    async fn prioritizes_incremental_bars_and_resumes_after_the_request_limit(
        mut harness: Harness,
    ) {
        let existing_id = "US:ZX-42";
        let missing_id = "US:ZX-43";
        let existing_latest = date("2040-01-02T00:00:00Z");
        let missing_latest = date("2040-01-04T11:59:00Z");
        let now = date("2040-01-04T12:00:00Z");
        let history_start = now
            .checked_sub_months(chrono::Months::new(INTRADAY_HISTORY_BOUNDARY_MONTHS))
            .unwrap_or(now);
        harness
            .repository
            .seed_us_stock_targets(vec![existing_id.to_owned(), missing_id.to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![bar(existing_id, Timeframe::Minute, existing_latest)])
            .await;
        harness.source.pagination = Some(PaginatedPages {
            instrument_id: missing_id.to_owned(),
            from: history_start,
            total_pages: MAX_REQUESTS_PER_RUN + 1,
            first_page_bars: vec![bar(missing_id, Timeframe::Minute, missing_latest)],
        });

        let first_stats = harness
            .use_cases
            .ingest_us_stock_minute_bars(&harness.source, now)
            .await
            .expect("bounded ingestion succeeds");
        harness.source.pagination = None;
        let second_stats = harness
            .use_cases
            .ingest_us_stock_minute_bars(&harness.source, now)
            .await
            .expect("resumed ingestion succeeds");

        let expected_calls = vec![
            BarQueryRun {
                instrument_ids: vec![existing_id.to_owned()],
                timeframe: Timeframe::Minute,
                from: existing_latest,
                to: now,
                page_count: 1,
                first_page_token: None,
                last_page_token: None,
                page_tokens_advance: true,
            },
            BarQueryRun {
                instrument_ids: vec![missing_id.to_owned()],
                timeframe: Timeframe::Minute,
                from: history_start,
                to: now,
                page_count: MAX_REQUESTS_PER_RUN - 1,
                first_page_token: None,
                last_page_token: Some((MAX_REQUESTS_PER_RUN - 2).to_string()),
                page_tokens_advance: true,
            },
            BarQueryRun {
                instrument_ids: vec![missing_id.to_owned(), existing_id.to_owned()],
                timeframe: Timeframe::Minute,
                from: missing_latest,
                to: now,
                page_count: 1,
                first_page_token: None,
                last_page_token: None,
                page_tokens_advance: true,
            },
        ];
        let (_, mut stored_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());
        stored_bars.sort_by_key(|bar| (bar.instrument_id.clone(), bar.timestamp));

        assert_eq!(
            (
                first_stats,
                second_stats,
                summarize_query_runs(&harness.source.calls.lock().await),
                stored_bars,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 2,
                    requests_attempted: MAX_REQUESTS_PER_RUN,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 2,
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: Some("request_budget_exhausted".to_owned()),
                    compression_error: None,
                },
                UsStockBarsIngestStats {
                    symbols_attempted: 2,
                    requests_attempted: 1,
                    daily_bars_upserted: 0,
                    minute_bars_upserted: 2,
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
                },
                expected_calls,
                vec![
                    bar(existing_id, Timeframe::Minute, existing_latest),
                    bar(existing_id, Timeframe::Minute, now),
                    bar(missing_id, Timeframe::Minute, missing_latest),
                    bar(missing_id, Timeframe::Minute, now),
                ],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn does_not_persist_partial_split_history_when_the_request_limit_is_reached(
        mut harness: Harness,
    ) {
        let instrument_id = "US:ZX-44";
        let earliest_daily = date("2039-12-31T00:00:00Z");
        let ex_date = date("2040-01-01T00:00:00Z");
        let latest_daily = date("2040-01-02T00:00:00Z");
        let now = date("2040-01-04T12:00:00Z");
        let replacement_bar = priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 100);
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
            ])
            .await;
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: ex_date.date_naive(),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });
        harness.source.pagination = Some(PaginatedPages {
            instrument_id: instrument_id.to_owned(),
            from: earliest_daily,
            total_pages: MAX_REQUESTS_PER_RUN,
            first_page_bars: vec![replacement_bar],
        });

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("partial split ingestion succeeds");
        let (mut stored_daily_bars, minute_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());
        stored_daily_bars.sort_by_key(|bar| bar.timestamp);
        let history_start = date("1970-01-01T00:00:00Z");
        let expected_calls = vec![
            BarQueryRun {
                instrument_ids: vec![instrument_id.to_owned()],
                timeframe: Timeframe::Daily,
                from: latest_daily,
                to: now,
                page_count: 1,
                first_page_token: None,
                last_page_token: None,
                page_tokens_advance: true,
            },
            BarQueryRun {
                instrument_ids: vec![instrument_id.to_owned()],
                timeframe: Timeframe::Daily,
                from: earliest_daily,
                to: now,
                page_count: MAX_REQUESTS_PER_RUN - 2,
                first_page_token: None,
                last_page_token: Some((MAX_REQUESTS_PER_RUN - 3).to_string()),
                page_tokens_advance: true,
            },
        ];

        assert_eq!(
            (
                stats,
                summarize_query_runs(&harness.source.calls.lock().await),
                harness.source.split_calls.lock().await.clone(),
                stored_daily_bars,
                minute_bars,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: MAX_REQUESTS_PER_RUN,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: Some("request_budget_exhausted".to_owned()),
                    compression_error: None,
                },
                expected_calls,
                vec![UsStockSplitQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    from: history_start.date_naive(),
                    to: now.date_naive(),
                }],
                vec![
                    priced_bar(instrument_id, Timeframe::Daily, earliest_daily, 200),
                    priced_bar(instrument_id, Timeframe::Daily, ex_date, 200),
                    priced_bar(instrument_id, Timeframe::Daily, latest_daily, 200),
                    bar(instrument_id, Timeframe::Daily, now),
                ],
                vec![],
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn isolates_rejected_symbols_when_fetching_split_history(mut harness: Harness) {
        let rejected_id = "US:QZ-REJECTED";
        let supported_id = "US:QZ-SUPPORTED";
        let ex_date = date("2040-01-01T00:00:00Z");
        let now = date("2040-01-03T12:00:00Z");
        harness
            .repository
            .seed_us_stock_daily_targets(vec![rejected_id.to_owned(), supported_id.to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![priced_bar(
                supported_id,
                Timeframe::Daily,
                ex_date,
                200,
            )])
            .await;
        harness.source.rejected_split_instruments = vec![rejected_id.to_owned()];
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: supported_id.to_owned(),
            ex_date: ex_date.date_naive(),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        let stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("one rejected split symbol does not fail daily ingestion");
        let actual = (
            stats,
            harness.source.calls.lock().await.clone(),
            harness.source.split_calls.lock().await.clone(),
            harness.repository.bars.lock().await.clone(),
        );

        assert_eq!(
            actual,
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 2,
                    requests_attempted: 6,
                    daily_bars_upserted: 3,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    skipped_instruments: vec![SkippedUsStockInstrument {
                        instrument_id: rejected_id.to_owned(),
                        reason: "synthetic split rejection".to_owned(),
                    }],
                    interrupted_reason: None,
                    compression_error: None,
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![supported_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: ex_date,
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![rejected_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: date("1970-01-01T00:00:00Z"),
                        to: now,
                        page_token: None,
                    },
                    UsStockBarQuery {
                        instrument_ids: vec![supported_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: ex_date,
                        to: now,
                        page_token: None,
                    },
                ],
                vec![
                    UsStockSplitQuery {
                        instrument_ids: vec![rejected_id.to_owned(), supported_id.to_owned()],
                        from: date("1970-01-01T00:00:00Z").date_naive(),
                        to: now.date_naive(),
                    },
                    UsStockSplitQuery {
                        instrument_ids: vec![rejected_id.to_owned()],
                        from: date("1970-01-01T00:00:00Z").date_naive(),
                        to: now.date_naive(),
                    },
                    UsStockSplitQuery {
                        instrument_ids: vec![supported_id.to_owned()],
                        from: date("1970-01-01T00:00:00Z").date_naive(),
                        to: now.date_naive(),
                    },
                ],
                vec![
                    priced_bar(supported_id, Timeframe::Daily, ex_date, 200),
                    bar(supported_id, Timeframe::Daily, now),
                    bar(rejected_id, Timeframe::Daily, now),
                ],
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
        let (mut daily_bars, mut minute_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());
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
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
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
    async fn split_refetch_retries_when_hourly_persistence_fails(harness: Harness) {
        let instrument_id = "US:DEMO-SPLIT";
        let earliest_daily = date("2040-01-01T00:00:00Z");
        let latest_daily = date("2040-01-02T00:00:00Z");
        let earliest_hourly = date("2039-12-31T23:00:00Z");
        let latest_hourly = date("2040-01-02T15:00:00Z");
        let now = date("2041-03-02T12:00:00Z");
        let hourly_end = date("2040-03-02T12:00:00Z");
        let old_progress = date("2040-02-01T00:00:00Z");
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![
                bar(instrument_id, Timeframe::Daily, earliest_daily),
                bar(instrument_id, Timeframe::Daily, latest_daily),
                bar(instrument_id, Timeframe::Hourly, earliest_hourly),
                bar(instrument_id, Timeframe::Hourly, latest_hourly),
            ])
            .await;
        harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .insert(instrument_id.to_owned(), old_progress);
        harness.source.bars.lock().await.extend([
            bar(instrument_id, Timeframe::Daily, earliest_daily),
            bar(instrument_id, Timeframe::Daily, latest_daily),
            bar(instrument_id, Timeframe::Daily, now),
            priced_bar(instrument_id, Timeframe::Hourly, earliest_hourly, 100),
            priced_bar(instrument_id, Timeframe::Hourly, latest_hourly, 100),
        ]);
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: earliest_daily.date_naive(),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        harness
            .repository
            .fail_hourly_upserts_with("synthetic hourly persistence failure")
            .await;
        let failed_stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("hourly persistence failure leaves the split pending");
        let failed_progress = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .get(instrument_id)
            .copied();
        let failed_ex_date_factor = harness
            .repository
            .bars
            .lock()
            .await
            .iter()
            .find(|bar| bar.timeframe == Timeframe::Daily && bar.timestamp == earliest_daily)
            .map(|bar| bar.adjustment_factor);
        let mut failed_hourly_bars = harness
            .repository
            .bars
            .lock()
            .await
            .iter()
            .filter(|bar| bar.timeframe == Timeframe::Hourly)
            .cloned()
            .collect::<Vec<_>>();
        failed_hourly_bars.sort_by_key(|bar| bar.timestamp);

        harness.source.calls.lock().await.clear();
        let retry_stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("the pending split is retried after hourly persistence recovers");
        let calls = harness.source.calls.lock().await.clone();
        let mut daily_bars = harness
            .repository
            .bars
            .lock()
            .await
            .iter()
            .filter(|bar| bar.timeframe == Timeframe::Daily)
            .cloned()
            .collect::<Vec<_>>();
        daily_bars.sort_by_key(|bar| bar.timestamp);
        let mut hourly_bars = harness
            .repository
            .bars
            .lock()
            .await
            .iter()
            .filter(|bar| bar.timeframe == Timeframe::Hourly)
            .cloned()
            .collect::<Vec<_>>();
        hourly_bars.sort_by_key(|bar| bar.timestamp);
        let completed_through = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .get(instrument_id)
            .copied();
        let mut adjusted_ex_date = bar(instrument_id, Timeframe::Daily, earliest_daily);
        adjusted_ex_date.adjustment_factor = Decimal::new(5, 1);

        assert_eq!(
            (
                failed_stats,
                failed_progress,
                failed_ex_date_factor,
                failed_hourly_bars,
                retry_stats,
                calls,
                daily_bars,
                hourly_bars,
                completed_through,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 4,
                    daily_bars_upserted: 2,
                    ..UsStockBarsIngestStats::default()
                },
                Some(old_progress),
                Some(Decimal::ONE),
                vec![
                    bar(instrument_id, Timeframe::Hourly, earliest_hourly),
                    bar(instrument_id, Timeframe::Hourly, latest_hourly),
                ],
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 4,
                    daily_bars_upserted: 4,
                    hourly_bars_upserted: 2,
                    ..UsStockBarsIngestStats::default()
                },
                vec![
                    UsStockBarQuery {
                        instrument_ids: vec![instrument_id.to_owned()],
                        timeframe: Timeframe::Daily,
                        from: now,
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
                        timeframe: Timeframe::Hourly,
                        from: earliest_hourly,
                        to: hourly_end,
                        page_token: None,
                    },
                ],
                vec![
                    adjusted_ex_date,
                    bar(instrument_id, Timeframe::Daily, latest_daily),
                    bar(instrument_id, Timeframe::Daily, now),
                ],
                vec![
                    priced_bar(instrument_id, Timeframe::Hourly, earliest_hourly, 100),
                    priced_bar(instrument_id, Timeframe::Hourly, latest_hourly, 100),
                ],
                Some(hourly_end),
            ),
        );
    }

    #[rstest]
    #[tokio::test]
    async fn split_refetch_keeps_skipped_hourly_history_retryable(mut harness: Harness) {
        let instrument_id = "US:QZ-SPLIT-SKIP";
        let earliest_daily = date("2040-01-01T00:00:00Z");
        let latest_daily = date("2040-01-02T00:00:00Z");
        let earliest_hourly = date("2039-12-31T23:00:00Z");
        let latest_hourly = date("2040-01-02T15:00:00Z");
        let now = date("2041-03-02T12:00:00Z");
        let hourly_end = date("2040-03-02T12:00:00Z");
        harness
            .repository
            .seed_us_stock_targets(vec![instrument_id.to_owned()])
            .await;
        harness
            .repository
            .seed_bars(vec![
                bar(instrument_id, Timeframe::Daily, earliest_daily),
                bar(instrument_id, Timeframe::Daily, latest_daily),
                bar(instrument_id, Timeframe::Hourly, earliest_hourly),
                bar(instrument_id, Timeframe::Hourly, latest_hourly),
            ])
            .await;
        harness.source.skipped_instruments = vec![instrument_id.to_owned()];
        harness.source.splits.lock().await.push(UsStockSplit {
            instrument_id: instrument_id.to_owned(),
            ex_date: earliest_daily.date_naive(),
            old_rate: Decimal::ONE,
            new_rate: Decimal::new(2, 0),
        });

        let split_stats = harness
            .use_cases
            .ingest_us_stock_daily_bars(&harness.source, now)
            .await
            .expect("split refetch can skip unsupported hourly symbols");
        let split_calls = harness.source.calls.lock().await.clone();
        let progress_after_split = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .get(instrument_id)
            .copied();
        harness.source.calls.lock().await.clear();
        let retry_stats = harness
            .use_cases
            .ingest_us_stock_hourly_bars(&harness.source, now)
            .await
            .expect("hourly job retries history skipped during split refetch");
        let retry_calls = harness.source.calls.lock().await.clone();
        let progress_after_retry = harness
            .repository
            .us_stock_hourly_bar_history_completed_through
            .lock()
            .await
            .get(instrument_id)
            .copied();

        assert_eq!(
            (
                split_stats,
                split_calls,
                progress_after_split,
                retry_stats,
                retry_calls,
                progress_after_retry,
            ),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 4,
                    skipped_instruments: vec![SkippedUsStockInstrument {
                        instrument_id: instrument_id.to_owned(),
                        reason: "synthetic symbol conversion failure".to_owned(),
                    }],
                    ..UsStockBarsIngestStats::default()
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
                        timeframe: Timeframe::Hourly,
                        from: earliest_hourly,
                        to: hourly_end,
                        page_token: None,
                    },
                ],
                None,
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 1,
                    skipped_instruments: vec![SkippedUsStockInstrument {
                        instrument_id: instrument_id.to_owned(),
                        reason: "synthetic symbol conversion failure".to_owned(),
                    }],
                    ..UsStockBarsIngestStats::default()
                },
                vec![UsStockBarQuery {
                    instrument_ids: vec![instrument_id.to_owned()],
                    timeframe: Timeframe::Hourly,
                    from: latest_hourly,
                    to: hourly_end,
                    page_token: None,
                }],
                None,
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
        let (daily_bars, minute_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());

        assert_eq!(
            (stats, calls, split_calls, daily_bars, minute_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 2,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
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
        let (mut daily_bars, mut minute_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());
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
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
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
        let (daily_bars, minute_bars) =
            partition_daily_bars(harness.repository.bars.lock().await.clone());
        assert_eq!(
            (stats, calls, split_calls, daily_bars, minute_bars),
            (
                UsStockBarsIngestStats {
                    symbols_attempted: 1,
                    requests_attempted: 2,
                    daily_bars_upserted: 1,
                    minute_bars_upserted: 0,
                    hourly_bars_upserted: 0,
                    skipped_instruments: Vec::new(),
                    interrupted_reason: None,
                    compression_error: None,
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
