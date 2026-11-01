use core_application::ingest_status::FED_CALENDAR_EVENT_INGEST_JOB;

use super::{
    DAILY_TIMEOUT, calendar_event::ingest_calendar_events_with_macro_references, require_source,
    run_with_ingest_run_log_state,
};

pub struct FedCalendarEventIngest;

impl super::SchedulerJob for FedCalendarEventIngest {
    const IDENTIFIER: &'static str = FED_CALENDAR_EVENT_INGEST_JOB;

    fn run(state: crate::state::SchedulerState) -> super::JobFuture {
        Box::pin(async move {
            run_with_ingest_run_log_state(
                state,
                <Self as super::SchedulerJob>::IDENTIFIER,
                DAILY_TIMEOUT,
                |state| async move {
                    let source = require_source(
                        state.dependencies.fed_calendar_source,
                        "Federal Reserve calendar event source",
                    )?;
                    ingest_calendar_events_with_macro_references(
                        &state.dependencies.calendar_events,
                        source.as_ref(),
                        state.dependencies.macro_references.as_ref(),
                    )
                    .await
                },
            )
            .await
        })
    }
}
