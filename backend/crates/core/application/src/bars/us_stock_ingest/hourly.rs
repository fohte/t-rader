use std::collections::HashSet;

use chrono::{DateTime, Utc};

use super::super::error::BarsUseCaseError;
use super::super::use_cases::UsStockBarsIngestUseCases;

impl UsStockBarsIngestUseCases {
    pub(super) async fn mark_hourly_history_completed(
        &self,
        instrument_ids: &[String],
        completed_through: DateTime<Utc>,
        skipped_instrument_ids: &HashSet<String>,
    ) -> Result<(), BarsUseCaseError> {
        let completed_instrument_ids = instrument_ids
            .iter()
            .filter(|instrument_id| !skipped_instrument_ids.contains(*instrument_id))
            .cloned()
            .collect::<Vec<_>>();
        if !completed_instrument_ids.is_empty() {
            self.repository
                .mark_us_stock_hourly_bar_history_completed(
                    &completed_instrument_ids,
                    completed_through,
                )
                .await?;
        }
        Ok(())
    }
}
