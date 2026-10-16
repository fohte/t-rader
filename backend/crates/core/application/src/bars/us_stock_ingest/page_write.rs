use core_domain::bar::Bar;

use crate::bars::error::BarsUseCaseError;
use crate::bars::types::UsStockBarsIngestStats;
use crate::bars::use_cases::BarsUseCases;
use crate::bars::write_throttle::WriteThrottle;

use super::UsStockBarTimeframe;
use super::request_budget::RequestBudget;

impl BarsUseCases {
    pub(super) async fn persist_bars(
        &self,
        bars: Vec<Bar>,
        timeframe: UsStockBarTimeframe,
        write_throttle: &mut WriteThrottle,
        budget: &mut RequestBudget,
        stats: &mut UsStockBarsIngestStats,
    ) -> Result<bool, BarsUseCaseError> {
        if bars.is_empty() {
            return Ok(true);
        }
        if !self
            .wait_for_page_write(write_throttle, budget, stats)
            .await?
        {
            budget.mark_time_budget_exhausted();
            return Ok(false);
        }
        let count = bars.len();
        let transaction = self.unit_of_work.begin().await?;
        let write_started_at = self.clock.instant();
        self.repository.upsert_bars(&transaction, bars).await?;
        let commit_started_at = self.clock.instant();
        match timeframe {
            UsStockBarTimeframe::Daily => stats.daily_bars_upserted += count,
            UsStockBarTimeframe::Minute => stats.minute_bars_upserted += count,
            UsStockBarTimeframe::Hourly => stats.hourly_bars_upserted += count,
        }
        self.unit_of_work.commit(transaction).await?;
        let completed_at = self.clock.instant();
        let write_duration = completed_at.saturating_duration_since(write_started_at);
        let commit_duration = completed_at.saturating_duration_since(commit_started_at);
        write_throttle.record_page(completed_at, write_duration, commit_duration);
        Ok(true)
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
                return Ok(true);
            };
            let commit_duration = ingest_run_log.probe_commit(*run_id).await?;
            write_throttle.record_probe_commit(self.clock.instant(), commit_duration);
        }
    }
}
