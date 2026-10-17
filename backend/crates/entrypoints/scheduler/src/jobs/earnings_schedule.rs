use chrono::Utc;
use core_application::{
    earnings_schedule::{
        EarningsScheduleIngestStats, EarningsScheduleUseCaseError, EarningsScheduleUseCases,
    },
    earnings_schedule_source::EarningsScheduleSource,
};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

pub struct EarningsScheduleIngest;

impl super::SchedulerJob for EarningsScheduleIngest {
    const IDENTIFIER: &'static str = core_application::ingest_status::EARNINGS_SCHEDULE_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.earnings_schedule_source,
                        "J-Quants earnings schedule source",
                    )?;
                    ingest_earnings_schedules(
                        &state.dependencies.earnings_schedules,
                        source.as_ref(),
                    )
                    .await
                },
            )
            .await
        })
    }
}

async fn ingest_earnings_schedules(
    use_cases: &EarningsScheduleUseCases,
    source: &dyn EarningsScheduleSource,
) -> Result<EarningsScheduleIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: EarningsScheduleUseCaseError| error.to_string())?;
    tracing::debug!(
        days_attempted = stats.days_attempted,
        upserted = stats.upserted,
        "earnings schedule ingest cycle completed"
    );
    Ok(stats)
}
