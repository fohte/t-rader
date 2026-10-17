use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use core_domain::bar::{Bar, Timeframe};
use rust_decimal::Decimal;

use super::super::error::BarsUseCaseError;
use super::super::types::BarsByInstrumentsQuery;
use super::super::use_cases::UsStockBarsIngestUseCases;

impl UsStockBarsIngestUseCases {
    pub(super) async fn preserve_adjustment_factors(
        &self,
        bars: &mut [Bar],
    ) -> Result<(), BarsUseCaseError> {
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
            .bars
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
