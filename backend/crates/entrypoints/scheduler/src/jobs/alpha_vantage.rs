use chrono::Utc;
use core_application::calendar::{
    error::CalendarEventUseCaseError,
    source::CalendarEventSource,
    use_cases::{CalendarEventIngestStats, CalendarEventUseCases},
};

use super::{DAILY_TIMEOUT, require_source, run_with_ingest_run_log_state};

pub struct AlphaVantageCalendarIngest;

impl super::SchedulerJob for AlphaVantageCalendarIngest {
    const IDENTIFIER: &'static str =
        core_application::ingest_status::ALPHA_VANTAGE_CALENDAR_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.alpha_vantage_calendar_source,
                        "Alpha Vantage calendar source",
                    )?;
                    ingest_calendar_events(&state.dependencies.calendar_events, source.as_ref())
                        .await
                },
            )
            .await
        })
    }
}

async fn ingest_calendar_events(
    use_cases: &CalendarEventUseCases,
    source: &dyn CalendarEventSource,
) -> Result<CalendarEventIngestStats, String> {
    let stats = use_cases
        .run_ingest_cycle(source, Utc::now().date_naive())
        .await
        .map_err(|error: CalendarEventUseCaseError| error.to_string())?;
    tracing::debug!(
        upserted = stats.upserted,
        deleted = stats.deleted,
        "Alpha Vantage calendar ingest cycle completed"
    );
    Ok(stats)
}
