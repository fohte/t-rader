use core_domain::bar::Bar;

use crate::bars::error::BarsUseCaseError;
use crate::bars::types::UsStockBarsIngestStats;
use crate::bars::use_cases::UsStockBarsIngestUseCases;
use crate::bars::write_throttle::WriteThrottle;

use super::request_budget::RequestBudget;
use super::{SIP_REFETCH_RUN_ROW_LIMIT_REACHED, SipRefetchPageWrite, UsStockBarTimeframe};

impl UsStockBarsIngestUseCases {
    pub(super) async fn persist_bars(
        &self,
        bars: Vec<Bar>,
        timeframe: UsStockBarTimeframe,
        write_throttle: &mut WriteThrottle,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
        sip_refetch_write: Option<SipRefetchPageWrite>,
    ) -> Result<bool, BarsUseCaseError> {
        let is_sip_refetch = sip_refetch_write.is_some();
        if bars.is_empty()
            && sip_refetch_write
                .as_ref()
                .is_none_or(|write| write.checkpoint.is_none())
        {
            return Ok(true);
        }
        let count = bars.len();
        if let Some(write) = sip_refetch_write.as_ref()
            && count > 0
        {
            let rows = u64::try_from(count).unwrap_or(u64::MAX);
            if stats.total_rows_upserted().saturating_add(rows) > write.max_rows_per_run {
                stats.interrupted_reason = Some(SIP_REFETCH_RUN_ROW_LIMIT_REACHED.to_owned());
                return Ok(false);
            }
        }
        if (!bars.is_empty() || is_sip_refetch)
            && !self
                .wait_for_page_write(write_throttle, budget, stats)
                .await?
        {
            budget.mark_time_budget_exhausted();
            return Ok(false);
        }
        if is_sip_refetch && !self.wait_for_sip_refetch_pacing(budget, stats).await? {
            return Ok(false);
        }
        let page_write_lock_guard = if is_sip_refetch {
            match self.acquire_us_stock_bars_write_lock(budget, stats).await? {
                Some(guard) => Some(guard),
                None => return Ok(false),
            }
        } else {
            None
        };
        let transaction = self.bars.unit_of_work.begin().await?;
        if let Some(write) = sip_refetch_write.as_ref() {
            let rows = u64::try_from(count).unwrap_or(u64::MAX);
            let pacing_millis = rows.saturating_mul(super::SIP_REFETCH_MILLIS_PER_ROW);
            if !self
                .bars
                .repository
                .reserve_us_stock_sip_refetch_rows(
                    &transaction,
                    rows,
                    write.max_rows_per_hour,
                    self.clock.utc_now(),
                    pacing_millis,
                )
                .await?
            {
                stats.interrupted_reason = Some("hourly_row_limit_reached".to_owned());
                return Ok(false);
            }
        }
        let write_started_at = self.clock.instant();
        if !bars.is_empty() {
            self.bars.repository.upsert_bars(&transaction, bars).await?;
            match timeframe {
                UsStockBarTimeframe::Daily => stats.daily_bars_upserted += count,
                UsStockBarTimeframe::Minute => stats.minute_bars_upserted += count,
                UsStockBarTimeframe::Hourly => stats.hourly_bars_upserted += count,
            }
        }
        if let Some(checkpoint) = sip_refetch_write.and_then(|write| write.checkpoint) {
            self.bars
                .repository
                .save_us_stock_sip_bar_refetch_cursor(
                    &transaction,
                    &checkpoint.instrument_id,
                    checkpoint.cursor,
                )
                .await?;
        }
        let commit_started_at = self.clock.instant();
        self.bars.unit_of_work.commit(transaction).await?;
        drop(page_write_lock_guard);
        let completed_at = self.clock.instant();
        if count > 0 {
            let write_duration = completed_at.saturating_duration_since(write_started_at);
            let commit_duration = completed_at.saturating_duration_since(commit_started_at);
            write_throttle.record_page(completed_at, write_duration, commit_duration);
        }
        Ok(true)
    }

    async fn wait_for_sip_refetch_pacing(
        &self,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        loop {
            if !budget.can_start_request(self.clock.instant()) {
                stats.interrupted_reason = budget.interrupted_reason.map(str::to_owned);
                return Ok(false);
            }
            let Some(next_allowed_at) = self
                .bars
                .repository
                .find_us_stock_sip_refetch_next_allowed_at()
                .await?
            else {
                return Ok(true);
            };
            let delay = next_allowed_at
                .signed_duration_since(self.clock.utc_now())
                .to_std()
                .unwrap_or_default();
            if delay.is_zero() {
                return Ok(true);
            }

            let sleep_started_at = self.clock.instant();
            let remaining = budget
                .deadline()
                .saturating_duration_since(sleep_started_at);
            self.clock.sleep(delay.min(remaining)).await;
            let waited = self
                .clock
                .instant()
                .saturating_duration_since(sleep_started_at);
            stats.write_throttle_wait_ms = stats
                .write_throttle_wait_ms
                .saturating_add(u64::try_from(waited.as_millis()).unwrap_or(u64::MAX));
            if self.clock.instant() >= budget.deadline() {
                budget.mark_time_budget_exhausted();
                stats.interrupted_reason = budget.interrupted_reason.map(str::to_owned);
                return Ok(false);
            }
        }
    }

    async fn wait_for_page_write(
        &self,
        write_throttle: &mut WriteThrottle,
        budget: &RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        loop {
            if !write_throttle
                .wait_before_write(self.clock.as_ref(), budget.deadline(), stats)
                .await
            {
                return Ok(false);
            }
            if !write_throttle.commit_needs_probe() {
                return Ok(true);
            }
            let Some((run_id, ingest_run_log)) = self.ingest_run.as_ref() else {
                return Err(BarsUseCaseError::IngestRunContextRequired);
            };
            match ingest_run_log.probe_commit(*run_id).await {
                Ok(commit_duration) => {
                    write_throttle.record_probe_commit(self.clock.instant(), commit_duration);
                }
                Err(error) => {
                    tracing::warn!(
                        %run_id,
                        %error,
                        "米国株バー取り込みの commit probe に失敗しました。前回の commit 時間を基準に再試行します"
                    );
                    write_throttle.record_probe_failure(self.clock.instant());
                }
            }
        }
    }
}
