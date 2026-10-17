use chrono::Utc;
use core_application::{
    bars::{BarsUseCaseError, BarsUseCases, IngestStats},
    daily_bar_source::DailyBarSource,
    market_daily_bar_source::MarketDailyBarSource,
};

use super::{
    DAILY_TIMEOUT, refresh_screener_snapshot, require_source, run_with_ingest_run_log_state,
    today_in_tokyo,
};

pub struct DailyBarsIngest;

impl super::SchedulerJob for DailyBarsIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::DAILY_BARS_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.market_daily_bar_source,
                        "J-Quants market daily bar source",
                    )?;
                    let instrument_source = require_source(
                        state.dependencies.daily_bar_source,
                        "J-Quants daily bar source",
                    )?;
                    let stats = ingest_daily_bars(
                        &state.dependencies.bars,
                        source.as_ref(),
                        instrument_source.as_ref(),
                    )
                    .await?;
                    refresh_screener_snapshot(&state.dependencies.screener_snapshot).await?;
                    Ok(stats)
                },
            )
            .await
        })
    }
}

async fn ingest_daily_bars(
    use_cases: &BarsUseCases,
    market_source: &dyn MarketDailyBarSource,
    instrument_source: &dyn DailyBarSource,
) -> Result<IngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(market_source, instrument_source, today_in_tokyo(Utc::now()))
        .await
        .map_err(|error: BarsUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        bars_upserted = stats.bars_upserted,
        "daily bars ingest cycle completed"
    );
    Ok(stats)
}
