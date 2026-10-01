use core_application::{
    bars::{BarsUseCaseError, BarsUseCases, IngestStats},
    market_daily_bar_source::MarketDailyBarSource,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct DailyBarsIngest;

impl TaskHandler for DailyBarsIngest {
    const IDENTIFIER: &'static str = "daily_bars_ingest";

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.market_daily_bar_source,
                    "J-Quants market daily bar source",
                )?;
                ingest_daily_bars(&state.dependencies.bars, source.as_ref()).await
            },
        )
        .await
    }
}

async fn ingest_daily_bars(
    use_cases: &BarsUseCases,
    source: &dyn MarketDailyBarSource,
) -> Result<IngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source)
        .await
        .map_err(|error: BarsUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        bars_upserted = stats.bars_upserted,
        "daily bars ingest cycle completed"
    );
    Ok(stats)
}
