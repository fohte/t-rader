use chrono::Utc;
use core_application::bars::{BarsUseCaseError, UsStockBarsIngestStats};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockBarsIngest;

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockMinuteBarsIngest;

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
                log_ingest_stats(
                    "1d",
                    state
                        .dependencies
                        .bars
                        .ingest_us_stock_daily_bars(source.as_ref(), Utc::now())
                        .await,
                )
            },
        )
        .await
    }
}

impl TaskHandler for UsStockMinuteBarsIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_MINUTE_BARS_INGEST_JOB;

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
                log_ingest_stats(
                    "1m",
                    state
                        .dependencies
                        .bars
                        .ingest_us_stock_minute_bars(source.as_ref(), Utc::now())
                        .await,
                )
            },
        )
        .await
    }
}

fn log_ingest_stats(
    bar_timeframe: &'static str,
    result: Result<UsStockBarsIngestStats, BarsUseCaseError>,
) -> Result<UsStockBarsIngestStats, String> {
    let stats = result.map_err(|error| error.to_string())?;
    tracing::debug!(
        bar_timeframe,
        symbols_attempted = stats.symbols_attempted,
        requests_attempted = stats.requests_attempted,
        daily_bars_upserted = stats.daily_bars_upserted,
        minute_bars_upserted = stats.minute_bars_upserted,
        skipped_instruments = stats.skipped_instruments.len(),
        interrupted_reason = ?stats.interrupted_reason,
        "US stock bar ingest cycle finished"
    );
    Ok(stats)
}
