use chrono::Utc;
use core_application::bars::{
    BarsUseCaseError, UsStockBarsIngestStats, UsStockMinuteBarsDownsampleStats,
};
use graphile_worker::{IntoTaskHandlerResult, TaskHandler, WorkerContext};
use serde::{Deserialize, Serialize};

use super::{DAILY_TIMEOUT, US_STOCK_BARS_TIMEOUT, require_source, run_with_ingest_run_log_state};

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockBarsIngest;

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockMinuteBarsIngest;

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockHourlyBarsIngest;

#[derive(Debug, Deserialize, Serialize)]
pub struct UsStockMinuteBarsDownsample;

impl TaskHandler for UsStockBarsIngest {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for UsStockBarsIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::US_STOCK_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                US_STOCK_BARS_TIMEOUT,
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
        })
    }
}

impl TaskHandler for UsStockMinuteBarsIngest {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for UsStockMinuteBarsIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_MINUTE_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                US_STOCK_BARS_TIMEOUT,
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
        })
    }
}

impl TaskHandler for UsStockHourlyBarsIngest {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for UsStockHourlyBarsIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_HOURLY_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                US_STOCK_BARS_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.us_stock_bar_source,
                        "Alpaca US stock bar source",
                    )?;
                    log_ingest_stats(
                        "1h",
                        state
                            .dependencies
                            .bars
                            .ingest_us_stock_hourly_bars(source.as_ref(), Utc::now())
                            .await,
                    )
                },
            )
            .await
        })
    }
}

impl TaskHandler for UsStockMinuteBarsDownsample {
    const IDENTIFIER: &'static str = <Self as super::SchedulerJob>::IDENTIFIER;

    async fn run(self, context: WorkerContext) -> impl IntoTaskHandlerResult {
        super::run_from_worker_context(context, <Self as super::SchedulerJob>::run).await
    }
}

impl super::SchedulerJob for UsStockMinuteBarsDownsample {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_MINUTE_BARS_DOWNSAMPLE_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let stats = state
                        .dependencies
                        .bars
                        .downsample_old_us_stock_minute_bars(Utc::now())
                        .await
                        .map_err(|error| error.to_string())?;
                    log_downsample_stats(stats)
                },
            )
            .await
        })
    }
}

fn log_ingest_stats(
    bar_timeframe: &'static str,
    result: Result<UsStockBarsIngestStats, BarsUseCaseError>,
) -> Result<UsStockBarsIngestStats, String> {
    let stats = result.map_err(|error| error.to_string())?;
    if let Some(error) = &stats.compression_error {
        tracing::error!(bar_timeframe, %error, "failed to compress old bars chunks after ingest");
    }
    tracing::debug!(
        bar_timeframe,
        symbols_attempted = stats.symbols_attempted,
        requests_attempted = stats.requests_attempted,
        daily_bars_upserted = stats.daily_bars_upserted,
        minute_bars_upserted = stats.minute_bars_upserted,
        hourly_bars_upserted = stats.hourly_bars_upserted,
        write_throttle_wait_ms = stats.write_throttle_wait_ms,
        skipped_instruments = stats.skipped_instruments.len(),
        interrupted_reason = ?stats.interrupted_reason,
        "US stock bar ingest cycle finished"
    );
    Ok(stats)
}

fn log_downsample_stats(
    stats: UsStockMinuteBarsDownsampleStats,
) -> Result<UsStockMinuteBarsDownsampleStats, String> {
    tracing::debug!(
        hourly_bars_inserted = stats.hourly_bars_inserted,
        minute_bars_deleted = stats.minute_bars_deleted,
        "US stock minute bar downsample cycle finished"
    );
    Ok(stats)
}
