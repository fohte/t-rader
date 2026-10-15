use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{DateTime, Months, NaiveDate, TimeZone, Utc};
use core_domain::bar::Timeframe;
use rust_decimal::Decimal;

use super::super::error::BarsUseCaseError;
use super::super::types::{BarsQuery, UsStockBarTarget, UsStockBarsIngestStats};
use super::super::us_stock_source::{
    UsStockBarSource, UsStockBarSourceError, UsStockSplit, UsStockSplitQuery,
};
use super::super::use_cases::BarsUseCases;
use super::{
    MINUTE_HISTORY_MONTHS, SYMBOLS_PER_REQUEST, UsStockBarTimeframe, UsStockSplitHistoryRange,
};

impl BarsUseCases {
    pub(super) async fn ingest_split_history(
        &self,
        source: &dyn UsStockBarSource,
        targets: &[UsStockBarTarget],
        from: NaiveDate,
        to: DateTime<Utc>,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        if targets.is_empty() {
            return Ok(());
        }

        let instrument_ids = targets
            .iter()
            .map(|target| target.instrument_id.clone())
            .collect::<Vec<_>>();
        let target_ids = instrument_ids.iter().cloned().collect::<HashSet<_>>();
        let mut splits_by_instrument: HashMap<String, Vec<UsStockSplit>> = HashMap::new();
        for instrument_ids in instrument_ids.chunks(SYMBOLS_PER_REQUEST) {
            let batch = source
                .fetch_splits(&UsStockSplitQuery {
                    instrument_ids: instrument_ids.to_vec(),
                    from,
                    to: to.date_naive(),
                })
                .await?;
            stats.requests_attempted += batch.requests_attempted;
            for split in batch.splits {
                if target_ids.contains(&split.instrument_id) && split.ex_date <= to.date_naive() {
                    splits_by_instrument
                        .entry(split.instrument_id.clone())
                        .or_default()
                        .push(split);
                }
            }
        }

        let daily_history_start = Utc.timestamp_opt(0, 0).single().unwrap_or(to);
        let minute_history_start = to
            .checked_sub_months(Months::new(MINUTE_HISTORY_MONTHS))
            .unwrap_or(to);
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
            if let Err(error) = self
                .refetch_split_history(
                    source,
                    target,
                    &pending_splits,
                    UsStockSplitHistoryRange {
                        daily_from: UsStockBarTimeframe::Daily
                            .earliest_bar(target)
                            .unwrap_or(daily_history_start),
                        minute_from: UsStockBarTimeframe::Minute
                            .earliest_bar(target)
                            .unwrap_or(minute_history_start),
                        to,
                    },
                    stats,
                )
                .await
            {
                tracing::error!(
                    instrument_id = %target.instrument_id,
                    error = %error,
                    "米国株の分割後履歴を再取得できませんでした"
                );
            }
        }

        Ok(())
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
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<(), BarsUseCaseError> {
        let instrument_ids = std::slice::from_ref(&target.instrument_id);
        let mut daily_pages = self
            .fetch_batch_pages(
                source,
                instrument_ids,
                UsStockBarTimeframe::Daily,
                range.daily_from,
                range.to,
                stats,
            )
            .await?;
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

        self.ingest_batch(
            source,
            instrument_ids,
            UsStockBarTimeframe::Minute,
            range.minute_from,
            range.to,
            stats,
        )
        .await?;

        self.persist_batch_pages(daily_pages, UsStockBarTimeframe::Daily, stats)
            .await?;
        self.persist_bars(split_bars, UsStockBarTimeframe::Daily, stats)
            .await
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
