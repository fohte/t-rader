use chrono::Utc;
use core_application::bars::{
    BarsUseCaseError, BarsUseCases, SharedUsStockBarSource, UsStockBarsIngestStats,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockBarsIngest;

impl TaskHandler for UsStockBarsIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::US_STOCK_BARS_INGEST_JOB;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        run_with_ingest_run_log_state(
            context,
            Self::IDENTIFIER,
            DAILY_TIMEOUT,
            |state| async move {
                let source = require_source(
                    state.dependencies.us_stock_bar_source,
                    "Alpaca US stock bar source",
                )?;
                ingest_us_stock_bars(&state.dependencies.bars, source, Utc::now()).await
            },
        )
        .await
    }
}

async fn ingest_us_stock_bars(
    use_cases: &BarsUseCases,
    source: SharedUsStockBarSource,
    now: chrono::DateTime<Utc>,
) -> Result<UsStockBarsIngestStats, String> {
    let stats = use_cases
        .ingest_us_stock_bars(source.as_ref(), now)
        .await
        .map_err(|error: BarsUseCaseError| error.to_string())?;
    tracing::debug!(
        symbols_attempted = stats.symbols_attempted,
        requests_attempted = stats.requests_attempted,
        daily_bars_upserted = stats.daily_bars_upserted,
        minute_bars_upserted = stats.minute_bars_upserted,
        "US stock bars ingest cycle completed"
    );
    Ok(stats)
}
