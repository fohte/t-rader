use chrono::Utc;
use core_application::bars::{
    US_STOCK_MINUTE_BARS_DOWNSAMPLE_PROCESSING_BUDGET, UsStockBarsIngestError,
    UsStockBarsIngestResult, UsStockMinuteBarsDownsampleConfig, UsStockMinuteBarsDownsampleStats,
};

use super::{
    DAILY_TIMEOUT, US_STOCK_BARS_TIMEOUT, refresh_screener_snapshot, require_source,
    run_with_ingest_run_log_state, run_with_ingest_run_log_state_with_run_id_and_failure_stats,
};

const MINUTE_BARS_DOWNSAMPLE_THROTTLING_ENABLED_ENV: &str =
    "US_STOCK_MINUTE_BARS_DOWNSAMPLE_THROTTLING_ENABLED";

pub struct UsStockBarsIngest;

pub struct UsStockMinuteBarsIngest;

pub struct UsStockHourlyBarsIngest;

pub struct UsStockMinuteBarsDownsample;

impl super::SchedulerJob for UsStockBarsIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::US_STOCK_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state_with_run_id_and_failure_stats(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                US_STOCK_BARS_TIMEOUT,
                |state, run_id| async move {
                    let bars = state
                        .dependencies
                        .us_stock_bars_ingest
                        .for_ingest_run(run_id, state.dependencies.ingest_run_log.clone());
                    let source = require_source(
                        state.dependencies.us_stock_bar_source,
                        "Alpaca US stock bar source",
                    )
                    .map_err(|error| (Box::new(UsStockBarsIngestResult::empty()), error))?;
                    let stats = log_ingest_stats(
                        "1d",
                        bars.ingest_us_stock_daily_bars(source.as_ref(), Utc::now())
                            .await,
                    )?;
                    match refresh_screener_snapshot(&state.dependencies.screener_snapshot).await {
                        Ok(()) => Ok(stats),
                        Err(error) => Err((Box::new(stats), error)),
                    }
                },
            )
            .await
        })
    }
}

impl super::SchedulerJob for UsStockMinuteBarsIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_MINUTE_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state_with_run_id_and_failure_stats(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                US_STOCK_BARS_TIMEOUT,
                |state, run_id| async move {
                    let bars = state
                        .dependencies
                        .us_stock_bars_ingest
                        .for_ingest_run(run_id, state.dependencies.ingest_run_log.clone());
                    let source = require_source(
                        state.dependencies.us_stock_bar_source,
                        "Alpaca US stock bar source",
                    )
                    .map_err(|error| (Box::new(UsStockBarsIngestResult::empty()), error))?;
                    log_ingest_stats(
                        "1m",
                        bars.ingest_us_stock_minute_bars(source.as_ref(), Utc::now())
                            .await,
                    )
                },
            )
            .await
        })
    }
}

impl super::SchedulerJob for UsStockHourlyBarsIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_HOURLY_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state_with_run_id_and_failure_stats(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                US_STOCK_BARS_TIMEOUT,
                |state, run_id| async move {
                    let bars = state
                        .dependencies
                        .us_stock_bars_ingest
                        .for_ingest_run(run_id, state.dependencies.ingest_run_log.clone());
                    let source = require_source(
                        state.dependencies.us_stock_bar_source,
                        "Alpaca US stock bar source",
                    )
                    .map_err(|error| (Box::new(UsStockBarsIngestResult::empty()), error))?;
                    log_ingest_stats(
                        "1h",
                        bars.ingest_us_stock_hourly_bars(source.as_ref(), Utc::now())
                            .await,
                    )
                },
            )
            .await
        })
    }
}

impl super::SchedulerJob for UsStockMinuteBarsDownsample {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::US_STOCK_MINUTE_BARS_DOWNSAMPLE_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            let config = downsample_config_from_env()?;
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let stats = state
                        .dependencies
                        .us_stock_bars_ingest
                        .downsample_old_us_stock_minute_bars(Utc::now(), config)
                        .await
                        .map_err(|error| error.to_string())?;
                    log_downsample_stats(stats)
                },
            )
            .await
        })
    }
}

fn downsample_config_from_env() -> Result<UsStockMinuteBarsDownsampleConfig, String> {
    let enabled = match std::env::var(MINUTE_BARS_DOWNSAMPLE_THROTTLING_ENABLED_ENV) {
        Ok(value) => parse_downsample_throttling_enabled(Some(value))?,
        Err(std::env::VarError::NotPresent) => parse_downsample_throttling_enabled(None)?,
        Err(error) => {
            return Err(format!(
                "failed to read {MINUTE_BARS_DOWNSAMPLE_THROTTLING_ENABLED_ENV}: {error}"
            ));
        }
    };
    Ok(downsample_config(enabled))
}

fn downsample_config(enabled: bool) -> UsStockMinuteBarsDownsampleConfig {
    UsStockMinuteBarsDownsampleConfig {
        max_processing_duration: enabled
            .then_some(US_STOCK_MINUTE_BARS_DOWNSAMPLE_PROCESSING_BUDGET),
    }
}

fn parse_downsample_throttling_enabled(value: Option<String>) -> Result<bool, String> {
    value
        .map(|value| {
            value.parse::<bool>().map_err(|_| {
                format!("{MINUTE_BARS_DOWNSAMPLE_THROTTLING_ENABLED_ENV} must be true or false")
            })
        })
        .transpose()
        .map(|enabled| enabled.unwrap_or(false))
}

pub(super) fn log_ingest_stats(
    bar_timeframe: &'static str,
    result: Result<UsStockBarsIngestResult, UsStockBarsIngestError>,
) -> Result<UsStockBarsIngestResult, (Box<UsStockBarsIngestResult>, String)> {
    let stats = result.map_err(|error| {
        let (stats, error) = error.into_parts();
        (Box::new(stats), error.to_string())
    })?;
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
        write_lock_wait_ms = stats.write_lock_wait_ms,
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

#[cfg(test)]
mod downsample_config_tests {
    use rstest::rstest;

    use super::{downsample_config, parse_downsample_throttling_enabled};
    use core_application::bars::{
        US_STOCK_MINUTE_BARS_DOWNSAMPLE_PROCESSING_BUDGET, UsStockMinuteBarsDownsampleConfig,
    };

    #[rstest]
    #[case::unset_disables_throttling(None, Ok(false))]
    #[case::true_enables_throttling(Some("true".to_owned()), Ok(true))]
    #[case::false_disables_throttling(Some("false".to_owned()), Ok(false))]
    #[case::invalid_values_are_rejected(Some("enabled".to_owned()), Err("US_STOCK_MINUTE_BARS_DOWNSAMPLE_THROTTLING_ENABLED must be true or false".to_owned()))]
    fn parses_the_throttling_setting(
        #[case] value: Option<String>,
        #[case] expected: Result<bool, String>,
    ) {
        assert_eq!(parse_downsample_throttling_enabled(value), expected);
    }

    #[rstest]
    #[case::off_uses_the_unlimited_default(false, None)]
    #[case::on_uses_the_approved_budget(
        true,
        Some(US_STOCK_MINUTE_BARS_DOWNSAMPLE_PROCESSING_BUDGET)
    )]
    fn maps_the_setting_to_the_downsample_budget(
        #[case] enabled: bool,
        #[case] max_processing_duration: Option<std::time::Duration>,
    ) {
        assert_eq!(
            downsample_config(enabled),
            UsStockMinuteBarsDownsampleConfig {
                max_processing_duration,
            },
        );
    }
}
