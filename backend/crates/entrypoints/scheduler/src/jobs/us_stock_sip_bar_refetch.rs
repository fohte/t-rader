use std::str::FromStr;

use chrono::Utc;
use core_application::bars::{
    DEFAULT_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR, MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR,
    UsStockBarsIngestResult, UsStockSipBarRefetchConfig,
};

use super::{
    US_STOCK_BARS_TIMEOUT, require_source,
    run_with_ingest_run_log_state_with_run_id_and_failure_stats, us_stock_bars::log_ingest_stats,
};

const MAX_SYMBOLS_ENV: &str = "US_STOCK_SIP_REFETCH_MAX_SYMBOLS";
const MAX_ROWS_PER_HOUR_ENV: &str = "US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR";

pub struct UsStockSipBarRefetch;

impl super::SchedulerJob for UsStockSipBarRefetch {
    const IDENTIFIER: &'static str = core_application::ingest_status::US_STOCK_SIP_BAR_REFETCH_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            let config = refetch_config_from_env()?;
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
                        "SIP refetch",
                        bars.refetch_us_stock_bars_via_sip(source.as_ref(), Utc::now(), config)
                            .await,
                    )
                },
            )
            .await
        })
    }
}

fn refetch_config_from_env() -> Result<UsStockSipBarRefetchConfig, String> {
    let max_symbols = parse_optional_positive::<usize>(MAX_SYMBOLS_ENV)?;
    let max_rows_per_hour = validate_max_rows_per_hour(
        parse_optional_positive::<u64>(MAX_ROWS_PER_HOUR_ENV)?
            .unwrap_or(DEFAULT_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR),
    )?;
    Ok(UsStockSipBarRefetchConfig {
        max_symbols,
        max_rows_per_hour,
    })
}

fn validate_max_rows_per_hour(value: u64) -> Result<u64, String> {
    if value < MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR {
        return Err(format!(
            "{MAX_ROWS_PER_HOUR_ENV} must be at least {MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR}"
        ));
    }
    Ok(value)
}

fn parse_optional_positive<T>(name: &str) -> Result<Option<T>, String>
where
    T: FromStr + PartialEq + Default,
{
    let value = match std::env::var(name) {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => return Err(format!("failed to read {name}: {error}")),
    };
    let parsed = value
        .parse::<T>()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if parsed == T::default() {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{MAX_ROWS_PER_HOUR_ENV, validate_max_rows_per_hour};
    use core_application::bars::MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR;

    #[rstest]
    #[case::minimum_is_accepted(
        MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR,
        Ok(MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR)
    )]
    #[case::larger_values_are_accepted(
        MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR + 1,
        Ok(MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR + 1)
    )]
    #[case::smaller_values_are_rejected(
        MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR - 1,
        Err(format!("{MAX_ROWS_PER_HOUR_ENV} must be at least {MIN_US_STOCK_SIP_REFETCH_MAX_ROWS_PER_HOUR}"))
    )]
    fn validates_the_minimum_page_size(#[case] value: u64, #[case] expected: Result<u64, String>) {
        assert_eq!(validate_max_rows_per_hour(value), expected);
    }
}
