use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use core_domain::bar::Timeframe;
use rust_decimal::Decimal;

use super::super::error::BarsUseCaseError;
use super::super::types::{BarsQuery, UsStockBarTarget, UsStockBarsIngestStats};
use super::super::us_stock_source::{
    UsStockBarSource, UsStockBarSourceError, UsStockSplit, UsStockSplitQuery,
};
use super::super::use_cases::BarsUseCases;
use super::{
    RequestBudget, SYMBOLS_PER_REQUEST, UsStockBarRange, UsStockBarTimeframe,
    UsStockSplitHistoryRange, record_skipped_instrument, split_rejected_batch,
};

impl BarsUseCases {
    pub(super) async fn ingest_split_history(
        &self,
        source: &dyn UsStockBarSource,
        targets: &[UsStockBarTarget],
        from: NaiveDate,
        to: DateTime<Utc>,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        if targets.is_empty() {
            return Ok(true);
        }

        let instrument_ids = targets
            .iter()
            .map(|target| target.instrument_id.clone())
            .collect::<Vec<_>>();
        let target_ids = instrument_ids.iter().cloned().collect::<HashSet<_>>();
        let mut splits_by_instrument: HashMap<String, Vec<UsStockSplit>> = HashMap::new();
        for instrument_ids in instrument_ids.chunks(SYMBOLS_PER_REQUEST) {
            let Some(batch_splits) = self
                .fetch_split_batch(source, instrument_ids, from, to.date_naive(), budget, stats)
                .await?
            else {
                return Ok(false);
            };
            for split in batch_splits {
                if target_ids.contains(&split.instrument_id) && split.ex_date <= to.date_naive() {
                    splits_by_instrument
                        .entry(split.instrument_id.clone())
                        .or_default()
                        .push(split);
                }
            }
        }

        for target in targets {
            let Some(splits) = splits_by_instrument.remove(&target.instrument_id) else {
                continue;
            };
            let pending_splits = match self
                .find_pending_splits(&target.instrument_id, splits)
                .await
            {
                Ok(pending_splits) => pending_splits,
                Err(error) => {
                    tracing::error!(
                        instrument_id = %target.instrument_id,
                        error = %error,
                        "米国株の分割履歴を確認できませんでした"
                    );
                    continue;
                }
            };
            if pending_splits.is_empty() {
                continue;
            }
            match self
                .refetch_split_history(
                    source,
                    target,
                    &pending_splits,
                    UsStockSplitHistoryRange {
                        daily_from: UsStockBarTimeframe::Daily
                            .earliest_bar(target)
                            .unwrap_or_else(|| UsStockBarTimeframe::Daily.history_start(to)),
                        minute_from: UsStockBarTimeframe::Minute
                            .earliest_bar(target)
                            .unwrap_or_else(|| UsStockBarTimeframe::Minute.history_start(to)),
                        to,
                    },
                    budget,
                    stats,
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => return Ok(false),
                Err(error) => {
                    tracing::error!(
                        instrument_id = %target.instrument_id,
                        error = %error,
                        "米国株の分割後履歴を再取得できませんでした"
                    );
                }
            }
        }

        Ok(true)
    }

    async fn fetch_split_batch(
        &self,
        source: &dyn UsStockBarSource,
        instrument_ids: &[String],
        from: NaiveDate,
        to: NaiveDate,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<Option<Vec<UsStockSplit>>, BarsUseCaseError> {
        let max_requests = budget.remaining();
        if max_requests == 0 {
            return Ok(None);
        }
        let result = source
            .fetch_splits(
                &UsStockSplitQuery {
                    instrument_ids: instrument_ids.to_vec(),
                    from,
                    to,
                },
                max_requests,
            )
            .await;
        match result {
            Ok(batch) => {
                if !budget.take(batch.requests_attempted) {
                    return Err(UsStockBarSourceError::Failed(
                        "source exceeded the request budget".to_owned(),
                    )
                    .into());
                }
                stats.requests_attempted += batch.requests_attempted;
                for skipped in batch.skipped_instruments {
                    record_skipped_instrument(stats, skipped.instrument_id, skipped.reason);
                }
                if batch.has_more {
                    return Ok(None);
                }
                Ok(Some(batch.splits))
            }
            Err(UsStockBarSourceError::RejectedSymbols(reason)) => {
                if !budget.take(1) {
                    return Ok(None);
                }
                stats.requests_attempted += 1;
                let Some((first, second)) = split_rejected_batch(instrument_ids, reason, stats)
                else {
                    return Ok(Some(Vec::new()));
                };
                let Some(mut first_splits) =
                    Box::pin(self.fetch_split_batch(source, first, from, to, budget, stats))
                        .await?
                else {
                    return Ok(None);
                };
                let Some(second_splits) =
                    Box::pin(self.fetch_split_batch(source, second, from, to, budget, stats))
                        .await?
                else {
                    return Ok(None);
                };
                first_splits.extend(second_splits);
                Ok(Some(first_splits))
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn find_pending_splits(
        &self,
        instrument_id: &str,
        splits: Vec<UsStockSplit>,
    ) -> Result<BTreeMap<NaiveDate, Decimal>, BarsUseCaseError> {
        let stored_bars = self
            .repository
            .find_bars(BarsQuery {
                instrument_id: instrument_id.to_owned(),
                timeframe: Timeframe::Daily.to_string(),
                from: None,
                to: None,
            })
            .await?;
        let stored_daily_range = stored_bars
            .iter()
            .map(|bar| bar.timestamp.date_naive())
            .min()
            .zip(
                stored_bars
                    .iter()
                    .map(|bar| bar.timestamp.date_naive())
                    .max(),
            );
        let stored_factors: HashMap<NaiveDate, Decimal> = stored_bars
            .into_iter()
            .map(|bar| (bar.timestamp.date_naive(), bar.adjustment_factor))
            .collect();
        let mut pending = BTreeMap::new();
        for split in splits {
            let Some((earliest_date, latest_date)) = stored_daily_range else {
                continue;
            };
            if split.ex_date < earliest_date || split.ex_date > latest_date {
                continue;
            }
            let factor = split_adjustment_factor(&split)?;
            if stored_factors
                .get(&split.ex_date)
                .is_some_and(|stored_factor| stored_factor == &factor)
            {
                continue;
            }
            if let Some(previous_factor) = pending.insert(split.ex_date, factor)
                && previous_factor != factor
            {
                return Err(UsStockBarSourceError::Failed(format!(
                    "conflicting split rates for {instrument_id} on {}",
                    split.ex_date
                ))
                .into());
            }
        }
        Ok(pending)
    }

    async fn refetch_split_history(
        &self,
        source: &dyn UsStockBarSource,
        target: &UsStockBarTarget,
        split_factors: &BTreeMap<NaiveDate, Decimal>,
        range: UsStockSplitHistoryRange,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        let instrument_ids = std::slice::from_ref(&target.instrument_id);
        let Some(mut daily_pages) = self
            .fetch_batch_pages(
                source,
                instrument_ids,
                UsStockBarTimeframe::Daily,
                UsStockBarRange {
                    from: range.daily_from,
                    to: range.to,
                },
                budget,
                stats,
            )
            .await?
        else {
            return Ok(false);
        };
        let mut split_bars = Vec::new();
        for page in &mut daily_pages {
            self.preserve_adjustment_factors(page).await?;
            let mut regular_bars = Vec::with_capacity(page.len());
            for mut bar in std::mem::take(page) {
                let date = bar.timestamp.date_naive();
                if let Some(factor) = split_factors.get(&date) {
                    bar.adjustment_factor = *factor;
                    split_bars.push(bar);
                } else {
                    regular_bars.push(bar);
                }
            }
            *page = regular_bars;
        }

        let minute_history_complete = if target.earliest_minute_bar.is_some() {
            self.ingest_batch(
                source,
                instrument_ids,
                UsStockBarTimeframe::Minute,
                UsStockBarRange {
                    from: range.minute_from,
                    to: range.to,
                },
                budget,
                stats,
            )
            .await?
        } else {
            true
        };
        if !minute_history_complete {
            return Ok(false);
        }

        self.persist_batch_pages(daily_pages, UsStockBarTimeframe::Daily, stats)
            .await?;
        self.persist_bars(split_bars, UsStockBarTimeframe::Daily, stats)
            .await?;
        Ok(true)
    }
}

fn split_adjustment_factor(split: &UsStockSplit) -> Result<Decimal, UsStockBarSourceError> {
    if split.old_rate <= Decimal::ZERO || split.new_rate <= Decimal::ZERO {
        return Err(UsStockBarSourceError::Failed(format!(
            "invalid split rates for {} on {}: {}/{}",
            split.instrument_id, split.ex_date, split.old_rate, split.new_rate
        )));
    }
    split.old_rate.checked_div(split.new_rate).ok_or_else(|| {
        UsStockBarSourceError::Failed(format!(
            "invalid split rates for {} on {}: {}/{}",
            split.instrument_id, split.ex_date, split.old_rate, split.new_rate
        ))
    })
}
